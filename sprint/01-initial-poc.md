# Initial POC

The initial proof of concept exposes three user actions:

## `serac set <dataset> [--file <input.tsv>]`

Stores one TSV dataset in the content-addressed column store.

1. Read TSV input (from `--file` or stdin) with headers enabled.
   - Fields are split on `\t`; rows are split on `\n`; CR is stripped for
     CRLF-tolerant line endings. There is no quoting layer (tabs/newlines
     must not appear inside fields).
2. Transpose rows into column vectors (`[][]const []const u8`).
3. Verify the first column is strictly sorted (error if not).
4. For each column:
   a. Serialize raw column values with the length-prefixed binary format.
   b. Compute SHA-256 and hex-encode it (this hash identifies the column).
   c. Resolve storage path as `.serac/<aa>/<bb>/<hex[4:]>`.
   d. If the file already exists, skip writing (cache hit).
   e. Otherwise zstd-compress and write the file.
5. Print a summary line for each column: `<column_name> -> <hex>`.
6. Update dataset index (`.serac/index`):
   a. Insert or update the dataset entry in sorted order.
   b. Re-encode and persist index columns: `names`, `headers`, `col_hashes`.

Checksums are computed on raw (serialized) column data, not compressed output,
so filenames remain deterministic across zstd versions and platforms.

Implementation notes (current code):

- `--file` and `-f` are both accepted for `set` and `get`.
- The strict sort check is lexicographic on raw UTF-8/byte slices of the first
  column (`a < b < c`, no duplicates).
- Empty lines are ignored while parsing TSV rows.
- The dataset index pointer file (`.serac/index`) contains exactly 3 newline-
  separated hashes in this order: `names`, `headers`, `col_hashes`.
- The length-prefixed binary codec uses little-endian `u64` counts/lengths.

## `serac get <dataset> [--file <output.tsv>]`

Reconstructs a TSV from previously stored columns.

1. Read the index and locate the requested dataset entry.
2. Load referenced column files from their hashes.
3. Decompress (zstd) and deserialize (length-prefixed binary) each column.
4. Rebuild TSV rows from columns, validating equal column lengths.
5. Write the reconstructed TSV to `--file` or stdout.

## `serac list`

Prints known dataset names.

1. Read the index (`.serac/index`).
2. Output stored dataset names in sorted order (one per line).

## Sprint-01 module split (implemented)

- `src/main.zig`: process/IO setup and usage output.
- `src/commands.zig`: CLI parsing/dispatch only.
- `src/service.zig`: business logic for `set/get/list`.
- `src/tsv.zig`: TSV parser/serializer.
- `src/codec.zig`: deterministic length-prefixed binary encoding.
- `src/storage.zig`: hash/compress/blob/index persistence.
