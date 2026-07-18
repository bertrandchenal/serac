# Tutorial: `src/service.zig`

This is the use-case/business layer. If you come from Python app
architecture, this is your service module that orchestrates parser +
storage + validation, without CLI concerns.

## Zig concepts used in this file

- **Layered design**: Service functions accept dependencies as
  parameters (`io`, allocator, repo path) instead of global
  state. This is closer to explicit dependency injection.
- **Domain validation**: Rules like strict first-column ordering live
  here. CLI and storage remain simpler because business invariants are
  centralized.
- **Struct return types**: `SetResult` is a typed bundle similar to a
  dataclass, but zero-runtime-reflection and compile-time checked.

## Function walkthrough (full code, tests omitted)

### `setFromTsv`

What to notice:
- Parses TSV, validates ordering invariant, writes column blobs.
- Updates dataset index with sorted upsert semantics.
- Returns per-column hashes for CLI summary output.

```zig
pub fn setFromTsv(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    dataset: []const u8,
    input_tsv: []const u8,
) !SetResult {
    const parsed = try tsv.parse(allocator, input_tsv);

    if (parsed.columns.len == 0) return error.EmptyInput;
    try verifyStrictlySorted(parsed.columns[0].items);

    const column_hashes = try allocator.alloc([]const u8, parsed.columns.len);
    for (parsed.columns, 0..) |column, idx| {
        const raw = try codec.encodeStringVec(allocator, column.items);
        column_hashes[idx] = try storage.writeEncodedColumn(io, allocator, repo_root, raw);
    }

    var index = try storage.readDatasetIndex(io, allocator, repo_root);
    index = try upsertDataset(allocator, index, dataset, parsed.headers, column_hashes);
    try storage.writeDatasetIndex(io, allocator, repo_root, index);

    return .{ .headers = parsed.headers, .hashes = column_hashes };
}
```

### `getAsTsv`

What to notice:
- Finds dataset metadata in index.
- Loads and decodes each stored column by hash.
- Rebuilds TSV by delegating serialization to `tsv.build`.

```zig
pub fn getAsTsv(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    dataset: []const u8,
) ![]u8 {
    const index = try storage.readDatasetIndex(io, allocator, repo_root);
    const pos = findName(index.names, dataset) orelse return error.DatasetNotFound;

    const headers = index.headers[pos];
    const hashes = index.col_hashes[pos];

    if (headers.len != hashes.len) return error.InvalidDatasetIndex;

    const columns = try allocator.alloc([][]const u8, hashes.len);
    for (hashes, 0..) |hash, idx| {
        const raw = try storage.readColumnRaw(io, allocator, repo_root, hash);
        columns[idx] = try codec.decodeStringVec(allocator, raw);
    }

    return tsv.build(allocator, headers, columns);
}
```

### `listDatasets`

What to notice:
- Thin pass-through to storage index read.
- Intentionally keeps listing logic simple and deterministic.

```zig
pub fn listDatasets(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
) ![]const []const u8 {
    const index = try storage.readDatasetIndex(io, allocator, repo_root);
    return index.names;
}
```

### `verifyStrictlySorted`

What to notice:
- Uses lexical byte ordering over `[]const u8` values.
- Rejects equal or descending neighbors to enforce strict monotonic order.

```zig
fn verifyStrictlySorted(values: []const []const u8) !void {
    if (values.len <= 1) return;

    var idx: usize = 1;
    while (idx < values.len) : (idx += 1) {
        if (std.mem.order(u8, values[idx - 1], values[idx]) != .lt) {
            return error.FirstColumnNotSorted;
        }
    }
}
```

### `upsertDataset`

What to notice:
- Implements replace-or-insert behavior while preserving sorted dataset names.
- Carefully clones slices because memory ownership is explicit in Zig.

```zig
fn upsertDataset(
    allocator: std.mem.Allocator,
    index: storage.DatasetIndex,
    dataset: []const u8,
    headers: [][]const u8,
    col_hashes: [][]const u8,
) !storage.DatasetIndex {
    if (headers.len != col_hashes.len) return error.InvalidDatasetIndex;

    if (findName(index.names, dataset)) |existing| {
        const names = try allocator.dupe([]const u8, index.names);
        const all_headers = try allocator.dupe([]const []const u8, index.headers);
        const all_hashes = try allocator.dupe([]const []const u8, index.col_hashes);

        names[existing] = try allocator.dupe(u8, dataset);
        all_headers[existing] = try codec.dupeStringVec(allocator, headers);
        all_hashes[existing] = try codec.dupeStringVec(allocator, col_hashes);

        return .{ .names = names, .headers = all_headers, .col_hashes = all_hashes };
    }

    const insert_at = findInsertPos(index.names, dataset);

    const names = try allocator.alloc([]const u8, index.names.len + 1);
    const all_headers = try allocator.alloc([]const []const u8, index.headers.len + 1);
    const all_hashes = try allocator.alloc([]const []const u8, index.col_hashes.len + 1);

    if (insert_at > 0) {
        @memcpy(names[0..insert_at], index.names[0..insert_at]);
        @memcpy(all_headers[0..insert_at], index.headers[0..insert_at]);
        @memcpy(all_hashes[0..insert_at], index.col_hashes[0..insert_at]);
    }

    names[insert_at] = try allocator.dupe(u8, dataset);
    all_headers[insert_at] = try codec.dupeStringVec(allocator, headers);
    all_hashes[insert_at] = try codec.dupeStringVec(allocator, col_hashes);

    if (insert_at < index.names.len) {
        @memcpy(names[insert_at + 1 ..], index.names[insert_at..]);
        @memcpy(all_headers[insert_at + 1 ..], index.headers[insert_at..]);
        @memcpy(all_hashes[insert_at + 1 ..], index.col_hashes[insert_at..]);
    }

    return .{ .names = names, .headers = all_headers, .col_hashes = all_hashes };
}
```

### `findName`

What to notice:
- Linear search helper returning optional index (`?usize`).

```zig
fn findName(names: []const []const u8, target: []const u8) ?usize {
    for (names, 0..) |name, idx| {
        if (std.mem.eql(u8, name, target)) return idx;
    }
    return null;
}
```

### `findInsertPos`

What to notice:
- Computes insertion point for sorted order maintenance.

```zig
fn findInsertPos(names: []const []const u8, target: []const u8) usize {
    var idx: usize = 0;
    while (idx < names.len) : (idx += 1) {
        if (std.mem.order(u8, target, names[idx]) == .lt) return idx;
    }
    return names.len;
}
```
