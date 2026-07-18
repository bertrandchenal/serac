# `src/repo.rs` - repository paths and index persistence

`repo.rs` owns filesystem-facing repository behavior.

## Responsibilities

- Define `Repo` and `DEFAULT_REPO_ROOT`.
- Emit file access traces through `log::debug!`.
- Resolve all repository paths (`column_path`, pointer files).
- Read/write `DatasetIndex` and `CollectionIndex` pointer files.
- Read/write content-addressed `Commit` files.
- Encode/decode content-addressed columns through zstd + bincode.

## Key API

- `Repo::new(root)`
- `Repo::default()` (uses `.risotto`)
- `repo.column_path(hex)`
- `repo.read_dataset_index()` / `repo.write_dataset_index(...)`
- `repo.read_collection_index()` / `repo.write_collection_index(...)`
- `repo.read_commit(hash)` / `repo.write_commit(...)`

## Notes

- Hashes are based on raw encoded bytes (deterministic naming).
- Pointer files remain three-line hash references for dataset/collection index.
- Commit logs are stored as content-addressed files whose payload references
  four content-addressed commit columns (`updated_at`, `min_value`,
  `max_value`, `hashes`).
- Repository file access is logged through `log::debug!`.
- Output formatting and routing are controlled by the logger initialized in
  `main` (`env_logger`).
- Validation is enforced before writes via shared index validation logic.

## Tests

Unit tests in this file cover path shaping and index pointer read/write behavior,
including missing and malformed pointer files.
