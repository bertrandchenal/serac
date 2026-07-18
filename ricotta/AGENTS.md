# ricotta

A CLI tool that reads a CSV, splits each column into an array,
computes a SHA-256 checksum of each column's raw data, compresses the
column with zstd, and writes the compressed content to a file named
after the checksum.


## Design decisions

| Decision       | Choice                                                                                                    |
|----------------|-----------------------------------------------------------------------------------------------------------|
| CSV parsing    | `csv` crate with headers enabled                                                                          |
| Checksum       | SHA-256 (hex) of **raw column values** encoded with bincode                                               |
| Compression    | zstd via the `zstd` crate (default level)                                                                 |
| Output file    | `.risotto/<2-char>/<2-char>/<sha256_hex[4:]>` — one file per column, sharded over two subdirectory levels |
| Error handling | `anyhow` for context-rich errors with automatic backtrace                                                 |
| CLI            | `ricotta set <dataset> [--file <input.csv>]` and `ricotta get <dataset> [--file <output.csv>]` via `clap` |

Checksums are computed on raw data (not compressed output) so
filenames are deterministic across zstd versions and platforms.

## Source tree

> Keep this tree up to date when files are added or removed. Tests
> live in the same file as the code under test and must be maintained
> alongside it. The `.rs.md` docs must be updated whenever the
> corresponding `.rs` file changes.

```
ricotta/
├── Cargo.toml          # crate manifest, dependencies
├── AGENTS.md           # design decisions, source tree
├── contrib/            # standalone experiments not used by the main CLI
│   ├── README.md       # experiment index and run instructions
│   └── blosc2-float-demo/
│       ├── Cargo.toml  # standalone demo manifest
│       └── src/
│           └── main.rs # compress a float vector with blosc2 and save to disk
├── sprint/
│   ├── 00-backlog.md               # deferred items and future ideas
│   ├── 01-initial-poc.md           # initial proof-of-concept behavior
│   ├── 02-collection-and-schema.md # collections and schema sprint details
│   ├── 03-repo-and-commit.md       # repo abstraction and commit sprint details
│   └── 04-concurrent-index-conflicts.md # concurrent write conflict scenarios at index level
├── sample.csv          # sample input for testing
└── src/
    ├── commands.rs      # high-level create/set/get/list orchestration; unit tests
    ├── commands.rs.md   # literate-programming walkthrough of commands.rs
    ├── lib.rs           # public facade, shared models/helpers, and re-exports
    ├── lib.rs.md        # literate-programming walkthrough of lib.rs
    ├── main.rs          # entry point, CLI parsing, thin wrapper around lib
    ├── main.rs.md       # literate-programming walkthrough of main.rs
    ├── repo.rs          # repository paths and index/content-addressed persistence
    └── repo.rs.md       # literate-programming walkthrough of repo.rs
```


## Literate-programming docs

Each `.rs` file has a sibling `.rs.md` file that explains the code in
literate-programming style.

| File             | Audience                                                                                       |
|------------------|------------------------------------------------------------------------------------------------|
| `src/commands.rs.md` | Covers high-level command orchestration for create/set/get/list and repo-aware variants     |
| `src/main.rs.md` | Covers CLI setup, `clap` derive macros, the `main` function, and `Result`-based error handling |
| `src/lib.rs.md`  | Covers shared data models/helpers, parsing, validation, hashing/compression utilities, facade design |
| `src/repo.rs.md` | Covers `Repo`, repository path resolution, and index pointer/content-addressed IO              |

Each concept (e.g. traits, the `?` operator, `Vec`, `String` vs `&str`) is
introduced at least once across the two files.
