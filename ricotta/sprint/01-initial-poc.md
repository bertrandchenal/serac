# Initial POC

The initial proof of concept exposes three user actions:

## `ricotta set <dataset> [--file <input.csv>]`

Stores one CSV dataset in the content-addressed column store.

1. Read CSV input (from `--file` or stdin) with headers enabled.
2. Transpose rows into column vectors (`Vec<Vec<String>>`).
3. Verify the first column is strictly sorted (error if not).
4. For each column:
   a. Serialize raw column values with bincode.
   b. Compute SHA-256 and hex-encode it (this hash identifies the column).
   c. Resolve storage path as `.ricotta/<aa>/<bb>/<hex[4:]>`.
   d. If the file already exists, skip writing (cache hit).
   e. Otherwise zstd-compress and write the file.
5. Print a summary line for each column: `<column_name> -> <hex>`.
6. Update dataset index (`.ricotta/index`):
   a. Insert or update the dataset entry in sorted order.
   b. Re-encode and persist index columns: `names`, `headers`, `col_hashes`.

Checksums are computed on raw (serialized) column data, not compressed output,
so filenames remain deterministic across zstd versions and platforms.

## `ricotta get <dataset> [--file <output.csv>]`

Reconstructs a CSV from previously stored columns.

1. Read the index and locate the requested dataset entry.
2. Load referenced column files from their hashes.
3. Decompress (zstd) and deserialize (bincode) each column.
4. Rebuild CSV rows from columns, validating equal column lengths.
5. Write the reconstructed CSV to `--file` or stdout.

## `ricotta list`

Prints known dataset names.

1. Read the index (`.ricotta/index`).
2. Output stored dataset names in sorted order (one per line).
