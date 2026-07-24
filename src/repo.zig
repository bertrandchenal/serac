//! Repository abstraction: paths and index/content-addressed persistence.
//!
//! Stub for now. Sprint 03 introduces the `Repo` struct, default `.serac`
//! root, and the `--repo` CLI flag.

const std = @import("std");

/// Default repository folder name.
pub const defaultRoot = ".serac";

/// Placeholder for the `Repo` type introduced in sprint 03.
pub const Repo = struct {
    pub fn open(allocator: std.mem.Allocator, root: []const u8) !Repo {
        _ = allocator;
        _ = root;
        return error.NotImplemented;
    }
};
