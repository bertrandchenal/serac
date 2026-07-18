# Repo and commit

This sprint introduces repository abstraction and append/update semantics.

## Goals

1. Stop hardcoding repository paths in code. Make repository root
   configurable from the CLI.
2. Preserve  dataset history

## Repo concept

- Add a `Repo` struct that owns the repository root path.
- Default repository folder is `.risotto`.
- Replace direct uses of hardcoded `.ricotta` paths with methods on `Repo`.
- `Repo` must expose constructors/helpers for index handles:
  - one to instantiate `CollectionIndex` access
  - one to instantiate `DatasetIndex` access

## CLI evolution

All commands accept an optional repository override:

- `--repo <path>` (default: `.risotto`)

Examples:

- `ricotta --repo .risotto-dev create temperature city temp`
- `ricotta --repo .risotto-dev set temperature/london --file input.csv`
- `ricotta --repo .risotto-dev get temperature/london --file output.csv`
- `ricotta --repo .risotto-dev list`

## Commit concept

Introduce a commit file, like other files it is column-oriented with
four columns:

- `updated_at`
- `min_value`
- `max_value`
- `hashes`

Each `set` writes a new commit file.

It is a copy of the previous commit file with a row appended (so the
file is sorted by `updated_at`. Each row references the column hashes
for one update range.

`get` reconstructs the full dataset by combining all commit rows in order.

## Index evolution in this sprint

`DatasetIndex` no longer stores direct `col_hashes`. It stores:

- `names`
- `collection_name`
- `commit_hash` (hash of the latest commit file for each dataset)

The latest commit file links to previous history so the full update chain can be
resolved at read time.

## Technical specification

### Repo

- `Repo` stores a root path (`PathBuf`).
- `Repo::new(root: impl Into<PathBuf>) -> Repo`.
- `Repo::default()` resolves to `.risotto`.
- All pointer files (`index`, `collections`, commit files, content-addressed
  blobs) must be resolved via `Repo` methods.

### Commit persistence

- Commit file is persisted using the same content-addressed mechanism as other
  special files.
- Commit columns are encoded as vectors and stored in a single
  content-addressed commit file whose hash is referenced by
  `DatasetIndex.commit_hash`.
- A commit row references one write operation (`set`) and includes min/max range
  metadata plus column hashes.

### Set/Get behavior

- `set`:
  1. validates collection/schema as before
  2. writes column blobs for incoming rows
  3. appends a commit row in a new commit file
  4. updates dataset index entry with the latest commit hash
- `get`:
  1. resolves dataset entry and latest commit hash
  2. traverses/loads commit history
  3. merges commit rows in order
  4. materializes final CSV

### Tests

- Tests must run with temporary repository roots (e.g. `.risotto-<random>`).
- Each test creates an isolated repo path and cleans it at the end.
- Remove shared hardcoded `.ricotta` filesystem state from tests.
