const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Core modules: each is a first-class named module exposed by the build.
    // `main.zig` and external consumers can `@import("commands")` /
    // `@import("repo")` directly — there is no umbrella facade.
    const repo_mod = b.addModule("repo", .{
        .root_source_file = b.path("src/repo.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });
    repo_mod.linkSystemLibrary("zstd", .{});

    const commands_mod = b.addModule("commands", .{
        .root_source_file = b.path("src/commands.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .imports = &.{
            .{ .name = "repo", .module = repo_mod },
        },
    });
    commands_mod.linkSystemLibrary("zstd", .{});

    // CLI executable: thin wrapper around the core modules.
    const exe = b.addExecutable(.{
        .name = "serac",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .imports = &.{
                .{ .name = "commands", .module = commands_mod },
                .{ .name = "repo", .module = repo_mod },
            },
        }),
    });
    // Link libzstd (system C library) for column compression.
    exe.root_module.linkSystemLibrary("zstd", .{});
    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| run_cmd.addArgs(args);

    const run_step = b.step("run", "Run the serac CLI");
    run_step.dependOn(&run_cmd.step);

    // Module tests (business logic, testable in isolation).
    const commands_tests = b.addTest(.{ .root_module = commands_mod });
    const run_commands_tests = b.addRunArtifact(commands_tests);

    const repo_tests = b.addTest(.{ .root_module = repo_mod });
    const run_repo_tests = b.addRunArtifact(repo_tests);

    // Executable tests (CLI parsing, dispatch).
    const exe_tests = b.addTest(.{ .root_module = exe.root_module });
    const run_exe_tests = b.addRunArtifact(exe_tests);

    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_commands_tests.step);
    test_step.dependOn(&run_repo_tests.step);
    test_step.dependOn(&run_exe_tests.step);
}
