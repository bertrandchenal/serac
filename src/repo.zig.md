# Tutorial: `src/repo.zig`

This is currently a Sprint 03 placeholder. It still matters because it
introduces a Zig `struct` with associated methods and a default
repository root constant.

## Zig concepts used in this file

- **`pub const`**: Compile-time constants are first-class and
  namespaced by module.
- **Methods on structs**: `Repo.open` is an associated function. Zig
  has no classes, but structs + functions provide the same
  composition.

## Function walkthrough (full code, tests omitted)

### `open`

What to notice:
- Current placeholder for future Repo abstraction; returns `NotImplemented`.

```zig
    pub fn open(allocator: std.mem.Allocator, root: []const u8) !Repo {
        _ = allocator;
        _ = root;
        return error.NotImplemented;
    }
```
