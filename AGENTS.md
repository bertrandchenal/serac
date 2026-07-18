# serac

A CLI tool that reads a TSV (tab-separated) file, splits each column
into an array, computes a SHA-256 checksum of each column's raw data,
compresses the column with zstd, and writes the compressed content to
a file named after the checksum.

Originally written in Rust (`ricotta/`, now archived) and being ported
to **Zig 0.16**.


## Design decisions

| Decision       | Choice                                                                                                       |
|----------------|--------------------------------------------------------------------------------------------------------------|
| Language       | Zig 0.16                                                                                                     |
| Input format   | TSV (tab-separated) — RFC 4180 minus the quoting state machine; simpler than CSV, same shape              |
| Parser         | Hand-rolled, ~20 lines of Zig (no external dep); splits on `\t` and `\n`, dedents CR for CRLF inputs       |
| Checksum       | SHA-256 (hex) of **raw column values** encoded with a length-prefixed binary format                          |
| Compression    | zstd via the system C library (`@cImport(@cInclude("zstd.h"))`)                                              |
| Output file    | `.serac/<2-char>/<2-char>/<sha256_hex[4:]>` — one file per column, sharded over two subdirectory levels      |
| Error handling | Zig error unions (`!T`); top-level `try` in `main.zig` propagates to the process exit code                    |
| CLI            | `serac set <dataset> [--file <input.tsv>]` / `serac get <dataset> [--file <output.tsv>]` / `serac list` via a thin hand-rolled arg parser |
| Build          | `zig build` (default), `zig build run`, `zig build test`                                                     |
| System libs    | libc + libzstd (linked via `linkSystemLibrary("zstd")` in `build.zig`)                                       |

Checksums are computed on raw data (not compressed output) so
filenames are deterministic across zstd versions and platforms.

## Source tree

> Keep this tree up to date when files are added or removed. Tests
> live in the same file as the code under test and must be maintained
> alongside it. Literate-programming `.zig.md` docs are introduced
> alongside each module as it stabilizes.

```
serac/
├── build.zig            # build configuration: target, optimize, zstd link
├── build.zig.zon        # package manifest (fingerprint, paths, deps)
├── AGENTS.md            # design decisions, source tree
├── .gitignore           # zig-out/, .zig-cache/, .serac/
├── contrib/             # standalone experiments not used by the main CLI
│   └── gen_tsv.py       # helper script to generate synthetic TSV datasets
├── sprint/
│   ├── 00-backlog.md               # deferred items and future ideas
│   ├── 01-initial-poc.md           # initial proof-of-concept behavior
│   ├── 02-collection-and-schema.md # collections and schema sprint details
│   ├── 03-repo-and-commit.md       # repo abstraction and commit sprint details
│   └── 04-concurrent-index-conflicts.md # concurrent write conflict scenarios at index level
├── sample.tsv           # sample input for testing
└── src/
    ├── main.zig         # CLI entry: arg parsing, stdout writer, calls `commands.dispatch`
    ├── main.zig.md      # literate doc for CLI entrypoint and process wiring
    ├── commands.zig     # CLI subcommand parsing and dispatch only
    ├── commands.zig.md  # literate doc for CLI parser/dispatcher responsibilities
    ├── service.zig      # business logic for `set` / `get` / `list`
    ├── service.zig.md   # literate doc for service-layer use cases
    ├── tsv.zig          # TSV parser/serializer utilities
    ├── tsv.zig.md       # literate doc for TSV format assumptions and behavior
    ├── codec.zig        # length-prefixed binary encoding/decoding helpers
    ├── codec.zig.md     # literate doc for binary format and determinism
    ├── storage.zig      # content-addressed storage and dataset index persistence
    ├── storage.zig.md   # literate doc for blob/index persistence details
    ├── repo.zig         # repository abstraction placeholder (`defaultRoot`, Sprint 03)
    └── repo.zig.md      # literate doc for repo abstraction roadmap
```


## Literate-programming docs

Each `.zig` file will eventually have a sibling `.zig.md` file that
explains the code in literate-programming style. Introduced
incrementally as each module stabilizes (post-sprint 01).

| File                | Audience                                                                                         |
|---------------------|--------------------------------------------------------------------------------------------------|
| `src/main.zig.md`   | CLI setup, arg parsing, the `main` function, and Zig's `!T` error-propagation style             |
| `src/commands.zig.md` | CLI parser/dispatcher: command syntax, flag handling, and IO wiring                            |
| `src/service.zig.md` | Dataset use-case logic for Sprint 01 (`set` / `get` / `list`) independent from CLI parsing     |
| `src/tsv.zig.md`    | TSV parsing/serialization assumptions and shape validation                                        |
| `src/codec.zig.md`  | Length-prefixed binary formats used for deterministic hashing and persistence                    |
| `src/storage.zig.md` | Content-addressed blob persistence, sharded paths, zstd, and dataset index pointer files       |
| `src/repo.zig.md`   | `Repo` abstraction roadmap and repository path resolution plan                                   |

Each concept (e.g. error unions, `std.ArrayList`, slices, `std.process.Init`,
`std.Io.Writer`) is introduced at least once across the docs.

## Status

- [x] Scaffold (`build.zig`, `build.zig.zon`, stubs in `src/`, `sample.tsv`)
- [x] Sprint 01 — initial POC: `set` / `get` / `list` end-to-end
- [ ] Sprint 02 — collections + schema
- [ ] Sprint 03 — `Repo` abstraction + commit history
- [ ] Sprint 04 — concurrent index conflict handling
