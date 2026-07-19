//! Content-addressed persistence primitives for Sprint 01.
//!
//! Handles:
//! - blob sharding path layout
//! - SHA-256 IDs
//! - zstd compression/decompression
//! - dataset index pointer file read/write

const std = @import("std");
const c = @cImport({
    @cInclude("zstd.h");
});
const codec = @import("codec.zig");

pub const Index = struct {
    names: []const []const u8,
    headers: []const []const []const u8,
    col_hashes: []const []const []const u8,
};

pub fn readIndex(io: std.Io, allocator: std.mem.Allocator, repo_root: []const u8) !Index {
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
        return error.InvalidIndex;
    }

    var idx: usize = 1;
    while (idx < names.len) : (idx += 1) {
        if (std.mem.order(u8, names[idx - 1], names[idx]) != .lt) {
            return error.InvalidIndex;
        }
    }

    return .{ .names = names, .headers = headers, .col_hashes = col_hashes };
}

pub fn writeIndex(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    index: Index,
) !void {
    if (index.names.len != index.headers.len or index.names.len != index.col_hashes.len) {
        return error.InvalidIndex;
    }

    var idx: usize = 1;
    while (idx < index.names.len) : (idx += 1) {
        if (std.mem.order(u8, index.names[idx - 1], index.names[idx]) != .lt) {
            return error.InvalidIndex;
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

fn columnPath(
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    hash: []const u8,
) ![]const u8 {
    if (hash.len < 5) return error.InvalidHash;
    return std.fs.path.join(allocator, &.{ repo_root, hash[0..2], hash[2..4], hash[4..] });
}

fn hashRawHex(allocator: std.mem.Allocator, raw: []const u8) ![]const u8 {
    var digest: [32]u8 = undefined;
    std.crypto.hash.sha2.Sha256.hash(raw, &digest, .{});
    const hex_array = std.fmt.bytesToHex(digest, .lower);
    return allocator.dupe(u8, &hex_array);
}

fn zstdCompress(allocator: std.mem.Allocator, raw: []const u8) ![]const u8 {
    const bound = c.ZSTD_compressBound(raw.len);
    const out = try allocator.alloc(u8, bound);

    const written = c.ZSTD_compress(out.ptr, out.len, raw.ptr, raw.len, 0);
    if (c.ZSTD_isError(written) != 0) return error.ZstdCompressFailed;

    return out[0..written];
}

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

fn readFile(io: std.Io, allocator: std.mem.Allocator, path: []const u8) ![]const u8 {
    return std.Io.Dir.cwd().readFileAlloc(io, path, allocator, .limited(std.math.maxInt(usize)));
}

fn fileExists(io: std.Io, path: []const u8) bool {
    _ = std.Io.Dir.cwd().access(io, path, .{}) catch return false;
    return true;
}

test "write/read dataset index roundtrip" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();

    const repo_root = try std.fmt.allocPrint(allocator, ".zig-cache/tmp/{s}/.serac-store", .{tmp.sub_path});

    const index = Index{
        .names = &[_][]const u8{ "a", "b" },
        .headers = &[_][]const []const u8{
            &[_][]const u8{ "h1", "h2" },
            &[_][]const u8{ "h1", "h2" },
        },
        .col_hashes = &[_][]const []const u8{
            &[_][]const u8{ "x1", "x2" },
            &[_][]const u8{ "y1", "y2" },
        },
    };

    try writeIndex(std.testing.io, allocator, repo_root, index);
    const loaded = try readIndex(std.testing.io, allocator, repo_root);

    try std.testing.expectEqual(@as(usize, 2), loaded.names.len);
    try std.testing.expectEqualStrings("a", loaded.names[0]);
    try std.testing.expectEqualStrings("y2", loaded.col_hashes[1][1]);
}
