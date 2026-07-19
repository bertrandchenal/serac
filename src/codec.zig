//! Binary encoding helpers for Serac's raw column payloads.
//!
//! Encoding format is length-prefixed and deterministic:
//! - `Vec<String>`: [u64 count][u64 len][bytes]...
//! - `Vec<Vec<String>>`: [u64 outer][u64 inner][u64 len][bytes]...

const std = @import("std");

fn encodeU64(out: *std.ArrayList(u8), value: u64, allocator: std.mem.Allocator) !void {
    var buf: [8]u8 = undefined;
    std.mem.writeInt(u64, &buf, value, .little);
    try out.appendSlice(allocator, &buf);
}

fn decodeU64(raw: []const u8, offset: *usize) !u64 {
    if (raw.len - offset.* < 8) return error.InvalidEncoding;
    const value = std.mem.readInt(u64, raw[offset.* .. offset.* + 8][0..8], .little);
    offset.* += 8;
    return value;
}

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

pub fn dupeStringVec(allocator: std.mem.Allocator, values: []const []const u8) ![][]const u8 {
    const out = try allocator.alloc([]const u8, values.len);
    for (values, 0..) |value, idx| {
        out[idx] = try allocator.dupe(u8, value);
    }
    return out;
}

test "encode/decode string vector roundtrip" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    const values = [_][]const u8{ "alice", "bob", "charlie" };

    const encoded = try encodeStringVec(allocator, &values);
    const decoded = try decodeStringVec(allocator, encoded);

    try std.testing.expectEqual(@as(usize, 3), decoded.len);
    try std.testing.expectEqualStrings("alice", decoded[0]);
    try std.testing.expectEqualStrings("bob", decoded[1]);
    try std.testing.expectEqualStrings("charlie", decoded[2]);
}

test "encode/decode matrix roundtrip" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    const matrix = [_][]const []const u8{
        &[_][]const u8{ "a", "b" },
        &[_][]const u8{ "c", "d" },
    };

    const encoded = try encodeStringMatrix(allocator, &matrix);
    const decoded = try decodeStringMatrix(allocator, encoded);

    try std.testing.expectEqual(@as(usize, 2), decoded.len);
    try std.testing.expectEqualStrings("a", decoded[0][0]);
    try std.testing.expectEqualStrings("d", decoded[1][1]);
}
