# Tutorial: `src/commands.zig`

This module is the CLI adapter layer. In Python terms, this is your
argparse/click command router, but hand-rolled and type-checked. It
should parse and route, not own business rules.

## Zig concepts used in this file

- **Slices**: `[]const u8` is a byte slice (pointer+len). You can
  think 'bytes view'. `[]const []const u8` is a list of byte slices
  (argv-style).
- **Optionals**: `?[]const u8` means value or null. Similar to
  `Optional[bytes]` in Python typing.
- **Control-flow as expressions**: Zig leans on explicit loops and
  branch conditions rather than hidden iterator magic. Command parsing
  here is straightforward index-walking.

## Function walkthrough (full code, tests omitted)

### `dispatch`

What to notice:
- Parses each subcommand manually for full control and zero dependencies.
- Returns `error.InvalidArguments` consistently for malformed inputs.
- Routes to service-layer functions, preserving separation of concerns.

```zig
pub fn dispatch(
    allocator: std.mem.Allocator,
    io: std.Io,
    args: []const []const u8,
    out: *std.Io.Writer,
) !void {
    if (args.len == 0) return error.InvalidArguments;

    if (std.mem.eql(u8, args[0], "set")) {
        if (args.len < 2) return error.InvalidArguments;

        const dataset = args[1];
        var input_file: ?[]const u8 = null;

        var i: usize = 2;
        while (i < args.len) : (i += 1) {
            const arg = args[i];
            if (std.mem.eql(u8, arg, "--file") or std.mem.eql(u8, arg, "-f")) {
                i += 1;
                if (i >= args.len) return error.InvalidArguments;
                input_file = args[i];
            } else {
                return error.InvalidArguments;
            }
        }

        const input_tsv = try readInputAll(io, allocator, input_file);
        const result = try service.setFromTsv(io, allocator, default_repo_root, dataset, input_tsv);

        var col_idx: usize = 0;
        while (col_idx < result.headers.len) : (col_idx += 1) {
            try out.print("{s} -> {s}\n", .{ result.headers[col_idx], result.hashes[col_idx] });
        }
        return;
    }

    if (std.mem.eql(u8, args[0], "get")) {
        if (args.len < 2) return error.InvalidArguments;

        const dataset = args[1];
        var output_file: ?[]const u8 = null;

        var i: usize = 2;
        while (i < args.len) : (i += 1) {
            const arg = args[i];
            if (std.mem.eql(u8, arg, "--file") or std.mem.eql(u8, arg, "-f")) {
                i += 1;
                if (i >= args.len) return error.InvalidArguments;
                output_file = args[i];
            } else {
                return error.InvalidArguments;
            }
        }

        const tsv_out = try service.getAsTsv(io, allocator, default_repo_root, dataset);

        if (output_file) |path| {
            try writeFile(io, path, tsv_out);
        } else {
            try out.writeAll(tsv_out);
        }
        return;
    }

    if (std.mem.eql(u8, args[0], "list")) {
        if (args.len != 1) return error.InvalidArguments;

        const names = try service.listDatasets(io, allocator, default_repo_root);
        for (names) |name| {
            try out.print("{s}\n", .{name});
        }
        return;
    }

    return error.InvalidArguments;
}
```

### `writeFile`

What to notice:
- Creates parent directories when needed.
- Uses truncating write semantics for deterministic output files.

```zig
fn writeFile(io: std.Io, path: []const u8, bytes: []const u8) !void {
    const cwd = std.Io.Dir.cwd();
    if (std.fs.path.dirname(path)) |parent| {
        try cwd.createDirPath(io, parent);
    }

    try cwd.writeFile(io, .{
        .sub_path = path,
        .data = bytes,
        .flags = .{ .truncate = true },
    });
}
```

### `readInputAll`

What to notice:
- Supports both file input and stdin streaming.
- Allocates full input into memory using the provided allocator.

```zig
fn readInputAll(io: std.Io, allocator: std.mem.Allocator, file_path: ?[]const u8) ![]const u8 {
    if (file_path) |path| {
        return std.Io.Dir.cwd().readFileAlloc(io, path, allocator, .limited(std.math.maxInt(usize)));
    }

    var stdin = std.Io.File.stdin();
    var reader = stdin.reader(io, &.{});
    return reader.interface.allocRemaining(allocator, .limited(std.math.maxInt(usize)));
}
```
