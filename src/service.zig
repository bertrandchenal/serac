//! Index-level business logic for Sprint 01 commands.
//!
//! This layer is independent from CLI parsing.

const std = @import("std");
const codec = @import("codec.zig");
const tsv = @import("tsv.zig");
const storage = @import("storage.zig");

pub const SetResult = struct {
    headers: []const []const u8,
    hashes: []const []const u8,
};

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

    var index = try storage.readIndex(io, allocator, repo_root);
    index = try upsertIndex(allocator, index, dataset, parsed.headers, column_hashes);
    try storage.writeIndex(io, allocator, repo_root, index);

    return .{ .headers = parsed.headers, .hashes = column_hashes };
}

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

fn verifyStrictlySorted(values: []const []const u8) !void {
    if (values.len <= 1) return;

    var idx: usize = 1;
    while (idx < values.len) : (idx += 1) {
        if (std.mem.order(u8, values[idx - 1], values[idx]) != .lt) {
            return error.FirstColumnNotSorted;
        }
    }
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

test "set/get/list roundtrip in isolated repo" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();

    const repo_root = try std.fmt.allocPrint(
        allocator,
        ".zig-cache/tmp/{s}/.serac-test",
        .{tmp.sub_path},
    );

    const input =
        "city\ttemp\n" ++
        "london\t15\n" ++
        "paris\t18\n";

    const result = try setFromTsv(std.testing.io, allocator, repo_root, "temperature", input);
    try std.testing.expectEqual(@as(usize, 2), result.headers.len);
    try std.testing.expectEqual(@as(usize, 2), result.hashes.len);

    const listed = try listIndex(std.testing.io, allocator, repo_root);
    try std.testing.expectEqual(@as(usize, 1), listed.len);
    try std.testing.expectEqualStrings("temperature", listed[0]);

    const output = try getAsTsv(std.testing.io, allocator, repo_root, "temperature");
    try std.testing.expectEqualStrings(input, output);
}

test "set rejects unsorted first column" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();

    const repo_root = try std.fmt.allocPrint(
        allocator,
        ".zig-cache/tmp/{s}/.serac-test",
        .{tmp.sub_path},
    );

    const input =
        "city\ttemp\n" ++
        "paris\t18\n" ++
        "london\t15\n";

    try std.testing.expectError(
        error.FirstColumnNotSorted,
        setFromTsv(std.testing.io, allocator, repo_root, "temperature", input),
    );
}
