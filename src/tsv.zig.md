# Tutorial: `src/tsv.zig`

This module handles TSV parsing/serialization only. It's intentionally
small and deterministic: no CSV quoting state machine, just
tabs/newlines with CRLF tolerance.

## Zig concepts used in this file

- **`std.ArrayList`**: Dynamic arrays require allocator-aware
  containers. `std.ArrayList(T)` is roughly Python `list[T]`, but
  explicit about allocation/deallocation.
- **Ownership + `deinit`**: When a struct owns allocations, it should
  provide a `deinit` method. This is a manual but predictable
  lifecycle model.
- **Error-first parsing**: Parser returns typed errors for malformed
  shape (`InvalidTsvShape`, `EmptyInput`) instead of exceptions with
  ad-hoc strings.

## Function walkthrough (full code, tests omitted)

### `deinit`

What to notice:
- Frees all allocations owned by `ParsedTsv`.
- Demonstrates explicit memory lifecycle management.

```zig
    pub fn deinit(self: *ParsedTsv, allocator: std.mem.Allocator) void {
        for (self.columns) |*col| col.deinit(allocator);
        allocator.free(self.columns);
        allocator.free(self.headers);
    }
```

### `parse`

What to notice:
- Parses header first, then transposes row-wise input into column-wise storage.
- Trims CR to accept CRLF files without a full CSV parser.

```zig
pub fn parse(allocator: std.mem.Allocator, input: []const u8) !ParsedTsv {
    var line_it = std.mem.splitScalar(u8, input, '\n');

    const first_line = line_it.next() orelse return error.EmptyInput;
    const header_line = trimCr(first_line);
    if (header_line.len == 0) return error.EmptyInput;

    const headers = try splitRowDup(allocator, header_line);
    if (headers.len == 0) return error.EmptyInput;

    const columns = try allocator.alloc(std.ArrayList([]const u8), headers.len);
    for (columns) |*col| col.* = .empty;

    while (line_it.next()) |line_raw| {
        const line = trimCr(line_raw);
        if (line.len == 0) continue;

        var field_it = std.mem.splitScalar(u8, line, '\t');
        var col_idx: usize = 0;
        while (field_it.next()) |field| : (col_idx += 1) {
            if (col_idx >= columns.len) return error.InvalidTsvShape;
            try columns[col_idx].append(allocator, try allocator.dupe(u8, field));
        }

        if (col_idx != columns.len) return error.InvalidTsvShape;
    }

    return .{ .headers = headers, .columns = columns };
}
```

### `build`

What to notice:
- Validates shape consistency before serialization.
- Serializes headers and rows with `	` and `
` delimiters.

```zig
pub fn build(
    allocator: std.mem.Allocator,
    headers: []const []const u8,
    columns: []const [][]const u8,
) ![]u8 {
    if (headers.len == 0) return error.InvalidTsvShape;
    if (columns.len != headers.len) return error.InvalidTsvShape;

    var row_count: usize = 0;
    if (columns.len > 0) row_count = columns[0].len;

    for (columns[1..]) |col| {
        if (col.len != row_count) return error.InvalidTsvShape;
    }

    var out: std.ArrayList(u8) = .empty;
    defer out.deinit(allocator);

    for (headers, 0..) |header, idx| {
        if (idx != 0) try out.append(allocator, '\t');
        try out.appendSlice(allocator, header);
    }
    try out.append(allocator, '\n');

    var row_idx: usize = 0;
    while (row_idx < row_count) : (row_idx += 1) {
        var col_idx: usize = 0;
        while (col_idx < columns.len) : (col_idx += 1) {
            if (col_idx != 0) try out.append(allocator, '\t');
            try out.appendSlice(allocator, columns[col_idx][row_idx]);
        }
        try out.append(allocator, '\n');
    }

    return out.toOwnedSlice(allocator);
}
```

### `trimCr`

What to notice:
- Small helper for CRLF compatibility.

```zig
fn trimCr(line: []const u8) []const u8 {
    if (line.len > 0 and line[line.len - 1] == '\r') return line[0 .. line.len - 1];
    return line;
}
```

### `splitRowDup`

What to notice:
- Splits by tabs and duplicates fields so lifetime is allocator-owned.

```zig
fn splitRowDup(allocator: std.mem.Allocator, line: []const u8) ![][]const u8 {
    var fields: std.ArrayList([]const u8) = .empty;
    defer fields.deinit(allocator);

    var it = std.mem.splitScalar(u8, line, '\t');
    while (it.next()) |field| {
        try fields.append(allocator, try allocator.dupe(u8, field));
    }

    return fields.toOwnedSlice(allocator);
}
```
