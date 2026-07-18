//! serac CLI entry point.
//!
//! Thin wrapper: parse args, dispatch to `commands.dispatch`, write results.
//! Concrete subcommand logic lives in `commands.zig`.

const std = @import("std");
const commands = @import("commands");

pub fn main(init: std.process.Init) !void {
    const arena = init.arena.allocator();
    const io = init.io;
    const args = try init.minimal.args.toSlice(arena);

    var stdout_buffer: [4096]u8 = undefined;
    var stdout_writer: std.Io.File.Writer = .init(.stdout(), io, &stdout_buffer);
    const out = &stdout_writer.interface;

    if (args.len < 2) {
        try printUsage(out);
        try out.flush();
        return;
    }

    try commands.dispatch(arena, io, args[1..], out);
    try out.flush();
}

fn printUsage(out: *std.Io.Writer) !void {
    try out.writeAll(
        \\serac — content-addressed column store for TSV files
        \\
        \\Usage:
        \\  serac <command> [args]
        \\
        \\Commands:
        \\  set <dataset> [--file <input.tsv>]   Store a TSV dataset
        \\  get <dataset> [--file <output.tsv>]  Reconstruct a TSV dataset
        \\  list                                 List known datasets
        \\
        \\Run `zig build run -- <command> [args]` to execute.
        \\
    );
}

test "printUsage writes expected command help" {
    var out: std.Io.Writer.Allocating = .init(std.testing.allocator);
    defer out.deinit();

    try printUsage(&out.writer);

    const usage = out.written();
    try std.testing.expect(std.mem.indexOf(u8, usage, "serac — content-addressed column store for TSV files") != null);
    try std.testing.expect(std.mem.indexOf(u8, usage, "set <dataset> [--file <input.tsv>]") != null);
    try std.testing.expect(std.mem.indexOf(u8, usage, "get <dataset> [--file <output.tsv>]") != null);
    try std.testing.expect(std.mem.indexOf(u8, usage, "list") != null);
}
