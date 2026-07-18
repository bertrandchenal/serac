# `src/lib.rs` - facade and shared core helpers

After the split, `lib.rs` is intentionally smaller and acts as the crate facade.

## Module wiring

`lib.rs` declares:

- `mod commands;`
- `mod repo;`

and re-exports the public surface:

- `pub use commands::*;`
- `pub use repo::{DEFAULT_REPO_ROOT, Repo};`

This keeps CLI and callers stable while implementation code lives in focused
modules.

## What stays in `lib.rs`

Shared domain and utility pieces used by both `commands` and `repo`:

- Dataset/schema/index models:
  - `DatasetKey`
  - `CollectionIndex`
  - `DatasetIndex`
  - `Commit`
- Validation utilities:
  - `Validate` trait
  - vector-length invariant helpers
  - schema validation (`validate_schema`)
- Encoding/compression/hash helpers:
  - `encode`
  - `hash_raw`
  - `compress_raw`
- CSV parsing/transposition:
  - `read_csv`
- Sorted first-column check:
  - `verify_sorted`

Compatibility wrappers for collection index IO are also kept in `lib.rs`:

- `read_collection_index*`
- `write_collection_index*`

These delegate to `Repo` methods.

## Why this split

- `repo.rs` owns filesystem persistence and repository path logic.
- `commands.rs` owns high-level set/get/create/list behavior.
- `lib.rs` owns shared types/helpers and stable exports.

This reduces file size and coupling without a large API redesign.

## Tests

Tests in `lib.rs` now focus on shared helpers and data-model validation.
Command and repository behavior tests moved to their respective modules.
