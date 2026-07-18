# Tutorial: `src/main.zig`

This file is the process entrypoint. Think of it like Python's `if
__name__ == "__main__":` block, but explicit and typed. It wires
runtime facilities (`io`, allocator, argv) and delegates real work.

## Zig concepts used in this file

- **Error unions (`!T`) + `try`**: Zig functions return either a value
  or an error. `!void` means success returns no value; failures return
  an error set. `try expr` propagates the error upward, like `raise`
  passthrough but explicit in type signatures.
- **Allocators instead of GC**: Instead of Python's garbage collector,
  Zig requires explicit allocation strategy. Here
  `init.arena.allocator()` gives an arena allocator suitable for
  process-lifetime allocations.
- **Explicit I/O objects**: `std.process.Init` gives `io` and
  args. Writers are concrete values (`std.Io.File.Writer`) with
  buffers you manage.

## Function walkthrough (full code, tests omitted)

### `main`

What to notice:
- Receives `std.process.Init` instead of using hidden globals.
- Builds a buffered stdout writer and flushes explicitly.
- Delegates command semantics to `commands.dispatch` to keep entrypoint tiny.

```zig
pub fn main(init: std.process.Init) !void {
    const arena = init.arena.allocator();
    const io = init.io;
    const args = try init.minimal.args.toSlice(arena);

    var stdout_buffer: [4096]u8 = undefined;
    var stdout_writer: std.Io.File.Writer = .init(.stdout(), io, &stdout_buffer);
    const out = &stdout_writer.interface;

    if (args.len < 2) {
        try printUsage(out);
        try out.flush();
        return;
    }

    try commands.dispatch(arena, io, args[1..], out);
    try out.flush();
}
```

### `printUsage`

What to notice:
- Takes a writer interface, so it's testable and reusable.
- Uses multiline string literal escaping (`\`) for readable CLI help text.

```zig
fn printUsage(out: *std.Io.Writer) !void {
    try out.writeAll(
        \\serac — content-addressed column store for TSV files
        \\
        \\Usage:
        \\  serac <command> [args]
        \\
        \\Commands:
        \\  set <dataset> [--file <input.tsv>]   Store a TSV dataset
        \\  get <dataset> [--file <output.tsv>]  Reconstruct a TSV dataset
        \\  list                                 List known datasets
        \\
        \\Run `zig build run -- <command> [args]` to execute.
        \\
    );
}
```
