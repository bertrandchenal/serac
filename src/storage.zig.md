# Tutorial: `src/storage.zig`

This module is persistence plumbing: hashing, sharded paths, zstd
compression, and index pointer files. It sits below service and above
raw filesystem primitives.

## Zig concepts used in this file

- **C interop**: `@cImport` exposes libzstd symbols directly. You call
  C functions with Zig types and handle error sentinels manually.
- **Content-addressed storage**: Blob ID is SHA-256(raw encoded
  bytes). The path is sharded by first 4 hex chars to avoid huge flat
  directories.
- **Filesystem I/O API**: `std.Io.Dir.cwd()` + methods
  (`readFileAlloc`, `writeFile`, `access`) are explicit and io-context
  aware.

## Function walkthrough (full code, tests omitted)

### `readDatasetIndex`

What to notice:
- Reads pointer file (`index`) then resolves each referenced blob.
- Validates shape and sorted-name invariants before returning.

```zig
pub fn readDatasetIndex(io: std.Io, allocator: std.mem.Allocator, repo_root: []const u8) !DatasetIndex {
    const index_path = try std.fs.path.join(allocator, &.{ repo_root, "index" });

    const pointer = readFile(io, allocator, index_path) catch |err| switch (err) {
        error.FileNotFound => {
            const names = try allocator.alloc([]const u8, 0);
            const headers = try allocator.alloc([][]const u8, 0);
            const col_hashes = try allocator.alloc([][]const u8, 0);
            return .{ .names = names, .headers = headers, .col_hashes = col_hashes };
        },
        else => return err,
    };

    var line_it = std.mem.splitScalar(u8, pointer, '\n');
    var hashes: [3][]const u8 = undefined;
    var count: usize = 0;

    while (line_it.next()) |line| {
        if (line.len == 0) continue;
        if (count >= 3) return error.InvalidIndexPointer;
        hashes[count] = line;
        count += 1;
    }

    if (count != 3) return error.InvalidIndexPointer;

    const names_raw = try readColumnRaw(io, allocator, repo_root, hashes[0]);
    const headers_raw = try readColumnRaw(io, allocator, repo_root, hashes[1]);
    const hashes_raw = try readColumnRaw(io, allocator, repo_root, hashes[2]);

    const names = try codec.decodeStringVec(allocator, names_raw);
    const headers = try codec.decodeStringMatrix(allocator, headers_raw);
    const col_hashes = try codec.decodeStringMatrix(allocator, hashes_raw);

    if (names.len != headers.len or names.len != col_hashes.len) {
        return error.InvalidDatasetIndex;
    }

    var idx: usize = 1;
    while (idx < names.len) : (idx += 1) {
        if (std.mem.order(u8, names[idx - 1], names[idx]) != .lt) {
            return error.InvalidDatasetIndex;
        }
    }

    return .{ .names = names, .headers = headers, .col_hashes = col_hashes };
}
```

### `writeDatasetIndex`

What to notice:
- Encodes and writes three index columns as content-addressed blobs.
- Stores only blob hashes in pointer file for compact indirection.

```zig
pub fn writeDatasetIndex(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    index: DatasetIndex,
) !void {
    if (index.names.len != index.headers.len or index.names.len != index.col_hashes.len) {
        return error.InvalidDatasetIndex;
    }

    var idx: usize = 1;
    while (idx < index.names.len) : (idx += 1) {
        if (std.mem.order(u8, index.names[idx - 1], index.names[idx]) != .lt) {
            return error.InvalidDatasetIndex;
        }
    }

    const names_raw = try codec.encodeStringVec(allocator, index.names);
    const headers_raw = try codec.encodeStringMatrix(allocator, index.headers);
    const hashes_raw = try codec.encodeStringMatrix(allocator, index.col_hashes);

    const names_hash = try writeEncodedColumn(io, allocator, repo_root, names_raw);
    const headers_hash = try writeEncodedColumn(io, allocator, repo_root, headers_raw);
    const hashes_hash = try writeEncodedColumn(io, allocator, repo_root, hashes_raw);

    const pointer = try std.fmt.allocPrint(allocator, "{s}\n{s}\n{s}\n", .{ names_hash, headers_hash, hashes_hash });
    const index_path = try std.fs.path.join(allocator, &.{ repo_root, "index" });
    try writeFile(io, index_path, pointer);
}
```

### `writeEncodedColumn`

What to notice:
- Hashes raw bytes, compresses with zstd, writes only on cache miss.

```zig
pub fn writeEncodedColumn(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    raw: []const u8,
) ![]const u8 {
    const hash = try hashRawHex(allocator, raw);
    const path = try columnPath(allocator, repo_root, hash);

    if (!fileExists(io, path)) {
        const compressed = try zstdCompress(allocator, raw);
        try writeFile(io, path, compressed);
    }

    return hash;
}
```

### `readColumnRaw`

What to notice:
- Resolves blob path by hash, reads compressed payload, decompresses bytes.

```zig
pub fn readColumnRaw(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    hash: []const u8,
) ![]const u8 {
    const path = try columnPath(allocator, repo_root, hash);
    const compressed = try readFile(io, allocator, path);
    return zstdDecompress(allocator, compressed);
}
```

### `columnPath`

What to notice:
- Implements `.serac/<aa>/<bb>/<rest>` sharding strategy.

```zig
fn columnPath(
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    hash: []const u8,
) ![]const u8 {
    if (hash.len < 5) return error.InvalidHash;
    return std.fs.path.join(allocator, &.{ repo_root, hash[0..2], hash[2..4], hash[4..] });
}
```

### `hashRawHex`

What to notice:
- Computes lowercase SHA-256 hex digest from raw bytes.

```zig
fn hashRawHex(allocator: std.mem.Allocator, raw: []const u8) ![]const u8 {
    var digest: [32]u8 = undefined;
    std.crypto.hash.sha2.Sha256.hash(raw, &digest, .{});
    const hex_array = std.fmt.bytesToHex(digest, .lower);
    return allocator.dupe(u8, &hex_array);
}
```

### `zstdCompress`

What to notice:
- Uses `ZSTD_compressBound` to allocate safe output capacity.
- Checks C API error sentinel with `ZSTD_isError`.

```zig
fn zstdCompress(allocator: std.mem.Allocator, raw: []const u8) ![]const u8 {
    const bound = c.ZSTD_compressBound(raw.len);
    const out = try allocator.alloc(u8, bound);

    const written = c.ZSTD_compress(out.ptr, out.len, raw.ptr, raw.len, 0);
    if (c.ZSTD_isError(written) != 0) return error.ZstdCompressFailed;

    return out[0..written];
}
```

### `zstdDecompress`

What to notice:
- Reads expected frame size using zstd metadata.
- Rejects unknown/invalid frame sizes for deterministic behavior.

```zig
fn zstdDecompress(allocator: std.mem.Allocator, compressed: []const u8) ![]const u8 {
    const expected = c.ZSTD_getFrameContentSize(compressed.ptr, compressed.len);
    const unknown = std.math.maxInt(@TypeOf(expected));
    const invalid = unknown - 1;
    if (expected == unknown or expected == invalid) {
        return error.ZstdUnknownSize;
    }

    const out = try allocator.alloc(u8, @as(usize, @intCast(expected)));
    const written = c.ZSTD_decompress(out.ptr, out.len, compressed.ptr, compressed.len);
    if (c.ZSTD_isError(written) != 0) return error.ZstdDecompressFailed;

    if (written != out.len) return error.ZstdSizeMismatch;
    return out;
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

### `readFile`

What to notice:
- Reads full file contents via `readFileAlloc` with explicit limit.

```zig
fn readFile(io: std.Io, allocator: std.mem.Allocator, path: []const u8) ![]const u8 {
    return std.Io.Dir.cwd().readFileAlloc(io, path, allocator, .limited(std.math.maxInt(usize)));
}
```

### `fileExists`

What to notice:
- Filesystem existence check via `access`, mapped to bool.

```zig
fn fileExists(io: std.Io, path: []const u8) bool {
    _ = std.Io.Dir.cwd().access(io, path, .{}) catch return false;
    return true;
}
```
