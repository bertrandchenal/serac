# `src/commands.rs` - high-level collection and dataset commands

`commands.rs` contains orchestration logic used by CLI-facing library functions.

## Responsibilities

- Implement collection lifecycle operations:
  - `create_collection*`
  - `list_collections*`
- Implement dataset operations:
  - `put_from_reader*` (set)
  - `get_to_writer*` (get)
  - `list_datasets*` and `list_datasets_in_collection*`
- Enforce schema/key constraints during set/get flows.

## Set flow

- Parse `<collection>/<dataset>` key.
- Verify first column sorted.
- Resolve collection schema and validate headers.
- Write column blobs content-addressed.
- Append a row to the dataset `Commit` (`updated_at`, `min_value`,
  `max_value`, `hashes`).
- Insert/update dataset metadata in sorted `DatasetIndex` using latest
  `commit_hash`.

## Get flow

- Parse key and resolve dataset entry.
- Resolve collection headers from `CollectionIndex`.
- Read commit log chain data from latest `commit_hash`.
- Decode all commit row columns in order and concatenate them.
- Reconstruct CSV rows and write to any `Write` target.

## Tests

Tests in this file cover end-to-end command behavior and key error conditions
for create/set/get/list operations.
