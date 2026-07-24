//! Index-level business logic for Sprint 01 commands.
//!
//! This layer is independent from CLI parsing.

const std = @import("std");
const codec = @import("codec.zig");
const storage = @import("storage.zig");
const utils = @import("utils.zig");

pub const Segment = struct {
    headers: [][]const u8,
    columns: []std.ArrayList([]const u8),

    pub fn deinit(self: *const Segment, allocator: std.mem.Allocator) void {
        for (self.columns) |*col| col.deinit(allocator);
        allocator.free(self.columns);
        allocator.free(self.headers);
    }

    pub fn validate(self: *const Segment) !void {
        // todo validate header len == nb cols
        if (self.columns.len == 0) return error.EmptyInput;
        try utils.verifyStrictlySorted(self.columns[0].items);
    }
};

pub fn setSegment(
    io: std.Io,
    allocator: std.mem.Allocator,
    repo_root: []const u8,
    dataset: []const u8,
    segment: *const Segment,
) !void {
    try segment.validate();

    const column_hashes = try allocator.alloc([]const u8, segment.columns.len);
    for (segment.columns, 0..) |column, idx| {
        const raw = try codec.encodeStringVec(allocator, column.items);
        column_hashes[idx] = try storage.writeEncodedColumn(io, allocator, repo_root, raw);
    }

    var index = try storage.readIndex(io, allocator, repo_root);
    index = try upsertIndex(allocator, index, dataset, segment.headers, column_hashes);
    try storage.writeIndex(io, allocator, repo_root, index);

    // var col_idx: usize = 0;
    // while (col_idx < result.headers.len) : (col_idx += 1) {
    //     try out.print("{s} -> {s}\n", .{ result.headers[col_idx], result.hashes[col_idx] });
    // }
}

fn upsertIndex(
    allocator: std.mem.Allocator,
    index: storage.Index,
    dataset: []const u8,
    headers: [][]const u8,
    col_hashes: [][]const u8,
) !storage.Index {
    if (headers.len != col_hashes.len) return error.InvalidIndex;

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

fn findName(names: []const []const u8, target: []const u8) ?usize {
    for (names, 0..) |name, idx| {
        if (std.mem.eql(u8, name, target)) return idx;
    }
    return null;
}

fn findInsertPos(names: []const []const u8, target: []const u8) usize {
    var idx: usize = 0;
    while (idx < names.len) : (idx += 1) {
        if (std.mem.order(u8, target, names[idx]) == .lt) return idx;
    }
    return names.len;
}
