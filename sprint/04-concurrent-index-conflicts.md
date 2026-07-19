# Concurrent index conflicts

This sprint note scopes conflict handling for concurrent `set` operations.

## Minimal data-loss scenario

Notation used in this scenario:

- `C0`, `C1`, `C2` are commit file hashes (commit identifiers), not column
  names.
- Each `C*` points to one commit file containing commit columns
  (`updated_at`, `min_value`, `max_value`, `hashes`).

The conflict manifests at index level (not at blob level): two writers read the
same `DatasetIndex` snapshot, then both publish different updates, and the last
index write wins.

Assume dataset key `temperature/paris` and initial state:

- `DatasetIndex.commit_hash[temperature/paris] = C0`

Timeline:

1. Process A starts `serac set temperature/paris --file a.tsv`.
2. Process B starts `serac set temperature/paris --file b.tsv`.
3. A reads `DatasetIndex` and sees `commit_hash = C0`.
4. B reads `DatasetIndex` and sees `commit_hash = C0`.
5. A writes new commit file with hash `C1` (append row based on `C0` history).
6. B writes new commit file with hash `C2` (append row based on `C0` history).
7. A writes `DatasetIndex.commit_hash[temperature/paris] = C1`.
8. B writes `DatasetIndex.commit_hash[temperature/paris] = C2`.

Final visible state is `C2`; commit `C1` becomes unreachable from the index.
No blob corruption occurs, but one successful write is lost from the logical
dataset history.
