//! CLI subcommand parsing and dispatch.
//!
//! This module intentionally keeps only argument parsing and IO wiring.
//! Business logic is delegated to `service.zig`.

const std = @import("std");
const repo = @import("repo");
const service = @import("service.zig");

const default_repo_root = repo.defaultRoot;

pub fn dispatch(
    allocator: std.mem.Allocator,
    io: std.Io,
    args: []const []const u8,
    out: *std.Io.Writer,
) !void {
    if (args.len == 0) return error.InvalidArguments;

    if (std.mem.eql(u8, args[0], "set")) {
        if (args.len < 2) return error.InvalidArguments;

        const dataset = args[1];
        var input_file: ?[]const u8 = null;

        var i: usize = 2;
        while (i < args.len) : (i += 1) {
            const arg = args[i];
            if (std.mem.eql(u8, arg, "--file") or std.mem.eql(u8, arg, "-f")) {
                i += 1;
                if (i >= args.len) return error.InvalidArguments;
                input_file = args[i];
            } else {
                return error.InvalidArguments;
            }
        }

        const input_tsv = try readInputAll(io, allocator, input_file);
        const result = try service.setFromTsv(io, allocator, default_repo_root, dataset, input_tsv);

        var col_idx: usize = 0;
        while (col_idx < result.headers.len) : (col_idx += 1) {
            try out.print("{s} -> {s}\n", .{ result.headers[col_idx], result.hashes[col_idx] });
        }
        return;
    }

    if (std.mem.eql(u8, args[0], "get")) {
        if (args.len < 2) return error.InvalidArguments;

        const dataset = args[1];
        var output_file: ?[]const u8 = null;

        var i: usize = 2;
        while (i < args.len) : (i += 1) {
            const arg = args[i];
            if (std.mem.eql(u8, arg, "--file") or std.mem.eql(u8, arg, "-f")) {
                i += 1;
                if (i >= args.len) return error.InvalidArguments;
                output_file = args[i];
            } else {
                return error.InvalidArguments;
            }
        }

        const tsv_out = try service.getAsTsv(io, allocator, default_repo_root, dataset);

        if (output_file) |path| {
            try writeFile(io, path, tsv_out);
        } else {
            try out.writeAll(tsv_out);
        }
        return;
    }

    if (std.mem.eql(u8, args[0], "list")) {
        if (args.len != 1) return error.InvalidArguments;

        const names = try service.listIndex(io, allocator, default_repo_root);
        for (names) |name| {
            try out.print("{s}\n", .{name});
        }
        return;
    }

    return error.InvalidArguments;
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

fn readInputAll(io: std.Io, allocator: std.mem.Allocator, file_path: ?[]const u8) ![]const u8 {
    if (file_path) |path| {
        return std.Io.Dir.cwd().readFileAlloc(io, path, allocator, .limited(std.math.maxInt(usize)));
    }

    var stdin = std.Io.File.stdin();
    var reader = stdin.reader(io, &.{});
    return reader.interface.allocRemaining(allocator, .limited(std.math.maxInt(usize)));
}

test "dispatch rejects unknown command" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const allocator = arena_state.allocator();

    var writer: std.Io.Writer.Discarding = .init(&.{});
    try std.testing.expectError(
        error.InvalidArguments,
        dispatch(allocator, std.testing.io, &.{"noop"}, &writer.writer),
    );
}
