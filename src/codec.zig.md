# Tutorial: `src/codec.zig`

This module defines deterministic binary encoding. In Python you'd
maybe use `struct.pack` + bytes joins; here we encode lengths and
payload bytes manually for stable hashing.

## Zig concepts used in this file

- **Binary format discipline**: Deterministic hashing requires
  deterministic bytes. We encode counts and lengths as little-endian
  `u64` to avoid ambiguity.
- **Casting with `@intCast`**: Zig does not silently narrow/widen
  integers. Conversions are explicit; this prevents many hidden bugs
  common in dynamic languages.
- **Bounds checks**: Decoders guard every read with length
  checks. Unsafe decode paths become explicit errors rather than UB or
  silent truncation.

## Function walkthrough (full code, tests omitted)

### `encodeU64`

What to notice:
- Writes little-endian integer bytes into dynamic buffer.

```zig
fn encodeU64(out: *std.ArrayList(u8), value: u64, allocator: std.mem.Allocator) !void {
    var buf: [8]u8 = undefined;
    std.mem.writeInt(u64, &buf, value, .little);
    try out.appendSlice(allocator, &buf);
}
```

### `decodeU64`

What to notice:
- Reads little-endian integer bytes with offset tracking and bounds checks.

```zig
fn decodeU64(raw: []const u8, offset: *usize) !u64 {
    if (raw.len - offset.* < 8) return error.InvalidEncoding;
    const value = std.mem.readInt(u64, raw[offset.* .. offset.* + 8][0..8], .little);
    offset.* += 8;
    return value;
}
```

### `encodeStringVec`

What to notice:
- Encodes vector length then each element length+bytes.

```zig
pub fn encodeStringVec(allocator: std.mem.Allocator, values: []const []const u8) ![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    defer out.deinit(allocator);

    try encodeU64(&out, @intCast(values.len), allocator);
    for (values) |value| {
        try encodeU64(&out, @intCast(value.len), allocator);
        try out.appendSlice(allocator, value);
    }

    return out.toOwnedSlice(allocator);
}
```

### `decodeStringVec`

What to notice:
- Reconstructs vector from deterministic length-prefixed format.

```zig
pub fn decodeStringVec(allocator: std.mem.Allocator, raw: []const u8) ![][]const u8 {
    var offset: usize = 0;
    const count: usize = @intCast(try decodeU64(raw, &offset));

    const out = try allocator.alloc([]const u8, count);
    var idx: usize = 0;
    while (idx < count) : (idx += 1) {
        const len: usize = @intCast(try decodeU64(raw, &offset));
        if (raw.len - offset < len) return error.InvalidEncoding;
        out[idx] = try allocator.dupe(u8, raw[offset .. offset + len]);
        offset += len;
    }

    if (offset != raw.len) return error.InvalidEncoding;
    return out;
}
```

### `encodeStringMatrix`

What to notice:
- Nested variant for matrix payloads (e.g., headers, hash groups).

```zig
pub fn encodeStringMatrix(allocator: std.mem.Allocator, rows: []const []const []const u8) ![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    defer out.deinit(allocator);

    try encodeU64(&out, @intCast(rows.len), allocator);
    for (rows) |row| {
        try encodeU64(&out, @intCast(row.len), allocator);
        for (row) |field| {
            try encodeU64(&out, @intCast(field.len), allocator);
            try out.appendSlice(allocator, field);
        }
    }

    return out.toOwnedSlice(allocator);
}
```

### `decodeStringMatrix`

What to notice:
- Nested decoder with strict bounds validation.

```zig
pub fn decodeStringMatrix(allocator: std.mem.Allocator, raw: []const u8) ![][][]const u8 {
    var offset: usize = 0;
    const outer: usize = @intCast(try decodeU64(raw, &offset));

    const rows = try allocator.alloc([][]const u8, outer);
    var i: usize = 0;
    while (i < outer) : (i += 1) {
        const inner: usize = @intCast(try decodeU64(raw, &offset));
        rows[i] = try allocator.alloc([]const u8, inner);

        var j: usize = 0;
        while (j < inner) : (j += 1) {
            const len: usize = @intCast(try decodeU64(raw, &offset));
            if (raw.len - offset < len) return error.InvalidEncoding;
            rows[i][j] = try allocator.dupe(u8, raw[offset .. offset + len]);
            offset += len;
        }
    }

    if (offset != raw.len) return error.InvalidEncoding;
    return rows;
}
```

### `dupeStringVec`

What to notice:
- Deep-copy helper for ownership-safe updates in index arrays.

```zig
pub fn dupeStringVec(allocator: std.mem.Allocator, values: []const []const u8) ![][]const u8 {
    const out = try allocator.alloc([]const u8, values.len);
    for (values, 0..) |value, idx| {
        out[idx] = try allocator.dupe(u8, value);
    }
    return out;
}
```
