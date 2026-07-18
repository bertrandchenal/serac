//! TSV parser/serializer utilities.
//!
//! Format assumptions (Sprint 01):
//! - tab-delimited fields (`\t`)
//! - newline-delimited rows (`\n`)
//! - optional CR in CRLF inputs is trimmed
//! - no quoting state machine

const std = @import("std");

pub const ParsedTsv = struct {
    headers: [][]const u8,
    columns: []std.ArrayList([]const u8),

    pub fn deinit(self: *ParsedTsv, allocator: std.mem.Allocator) void {
        for (self.columns) |*col| col.deinit(allocator);
        allocator.free(self.columns);
        allocator.free(self.headers);
    }
};

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

fn trimCr(line: []const u8) []const u8 {
    if (line.len > 0 and line[line.len - 1] == '\r') return line[0 .. line.len - 1];
    return line;
}

fn splitRowDup(allocator: std.mem.Allocator, line: []const u8) ![][]const u8 {
    var fields: std.ArrayList([]const u8) = .empty;
    defer fields.deinit(allocator);

    var it = std.mem.splitScalar(u8, line, '\t');
    while (it.next()) |field| {
        try fields.append(allocator, try allocator.dupe(u8, field));
    }

    return fields.toOwnedSlice(allocator);
}

test "parse and build TSV roundtrip" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    const input =
        "city\ttemp\n" ++
        "london\t15\n" ++
        "paris\t18\n";

    var parsed = try parse(allocator, input);
    const columns = try allocator.alloc([][]const u8, parsed.columns.len);
    for (parsed.columns, 0..) |column, idx| columns[idx] = column.items;

    const rebuilt = try build(allocator, parsed.headers, columns);
    try std.testing.expectEqualStrings(input, rebuilt);
    parsed.deinit(allocator);
}

test "parse rejects jagged row" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    const input =
        "city\ttemp\n" ++
        "london\n";

    try std.testing.expectError(error.InvalidTsvShape, parse(allocator, input));
}
