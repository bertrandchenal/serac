const std = @import("std");

pub fn verifyStrictlySorted(values: []const []const u8) !void {
    if (values.len <= 1) return;

    var idx: usize = 1;
    while (idx < values.len) : (idx += 1) {
        if (std.mem.order(u8, values[idx - 1], values[idx]) != .lt) {
            return error.FirstColumnNotSorted;
        }
    }
}
