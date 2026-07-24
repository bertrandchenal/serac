//! Index-level business logic for Sprint 01 commands.
//!
//! This layer is independent from CLI parsing.

const std = @import("std");
const codec = @import("codec.zig");
const storage = @import("storage.zig");
const tsv = @import("tsv.zig");

pub fn getAsTsv(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    dataset: []const u8,
) ![]u8 {
    const index = try storage.readIndex(io, allocator, repo_root);
    const pos = findName(index.names, dataset) orelse return error.IndexNotFound;

    const headers = index.headers[pos];
    const hashes = index.col_hashes[pos];

    if (headers.len != hashes.len) return error.InvalidIndex;

    const columns = try allocator.alloc([][]const u8, hashes.len);
    for (hashes, 0..) |hash, idx| {
        const raw = try storage.readColumnRaw(io, allocator, repo_root, hash);
        columns[idx] = try codec.decodeStringVec(allocator, raw);
    }

    return tsv.build(allocator, headers, columns);
}

pub fn listIndex(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
) ![]const []const u8 {
    const index = try storage.readIndex(io, allocator, repo_root);
    return index.names;
}

fn findName(names: []const []const u8, target: []const u8) ?usize {
    for (names, 0..) |name, idx| {
        if (std.mem.eql(u8, name, target)) return idx;
    }
    return null;
}
