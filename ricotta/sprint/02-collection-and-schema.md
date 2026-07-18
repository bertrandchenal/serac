# Collection and schema

Next step: introduce collections so datasets reference a schema by name instead
of duplicating headers in the index.

## Concepts

- **Dataset**: still the unit stored and retrieved by `set`/`get`.
- **Collection**: identifies a schema and can be shared by many datasets.
- **Schema**: ordered `headers` + ordered `types` (same length, same positions).

## Rules

1. Collection names are unique.
2. Different collection names may share identical schema (`headers` + `types`).
3. For now, supported column type is only `string`.
4. Dataset index entries store `collection_name` instead of per-dataset headers.

## Storage model

As with the index, collections are stored through three content-addressed
columns plus a pointer file.

- Collection columns:
  - `names` (collection names)
  - `headers` (header vectors per collection)
  - `types` (type vectors per collection)
- Pointer file:
  - `.ricotta/collections`
  - contains three hashes (one per column), same pattern as `.ricotta/index`

This keeps collections append/update friendly and deduplicates schema metadata
across datasets.

## Index evolution

Current index columns:

- `names`
- `headers`
- `col_hashes`

Target index columns:

- `names`
- `collection_name`
- `col_hashes`

`get` resolves a dataset by name, then resolves its `collection_name` to recover
`headers` (and later `types`) from `.ricotta/collections`.

## CLI evolution

Collections introduce one new command and namespace datasets under a
`<collection>/<dataset>` key.

- `ricotta create <collection> <header>...`
  - registers a collection with the provided ordered headers
  - for now all column types are implicitly `string`
- `ricotta set <collection>/<dataset> [--file <input.csv>]`
  - dataset name is namespaced by collection
  - validates CSV headers against the target collection schema before storing
- `ricotta get <collection>/<dataset> [--file <output.csv>]`
  - reads the dataset from the namespaced key and reconstructs CSV as before
- `ricotta list`
  - prints known collection names
- `ricotta list <collection>`
  - prints dataset names inside the target collection
