## `src/main.rs` — CLI entry point

`main.rs` defines a `clap` subcommand CLI and routes collection-aware commands
to library functions in `lib.rs`, with optional repository and debug flags.

### Imports

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::io;
use std::path::PathBuf;
```

- `anyhow::Result` gives a single error type for command handlers.
- `clap::{Parser, Subcommand}` derives the CLI parser.

### CLI shape

```rust
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(long = "repo", default_value = ricotta::DEFAULT_REPO_ROOT)]
    repo: PathBuf,

    #[arg(long = "debug")]
    debug: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Create {
        collection: String,
        headers: Vec<String>,
    },
    Set {
        name: String,
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
    },
    List {
        collection: Option<String>,
    },
    Get {
        name: String,
        #[arg(short = 'f', long = "file")]
        file: Option<PathBuf>,
    },
}
```

- `create` registers a collection schema.
- `set` ingests a CSV into the selected repository under a namespaced key.
- `get` reconstructs a stored dataset as CSV from a namespaced key.
- `list` prints collections or datasets inside one collection.
- `--repo <path>` overrides repository root (default `.risotto`).
- `--debug` enables debug-level logging.
- Both `set` and `get` use `-f`/`--file` for optional file paths.

### Main routing

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Create { collection, headers } => { ... }
        Command::Set { name, file } => { ... }
        Command::List { collection } => { ... }
        Command::Get { name, file } => { ... }
    }

    Ok(())
}
```

### `set` modes

- **File mode**: `ricotta set temperature/london --file input.csv`
  - reads CSV from file
- **stdin mode**: `cat data.csv | ricotta set temperature/london`
  - reads CSV from stdin

`set` calls `ricotta::put_from_reader_in_repo` in both modes:

- file mode opens the path and passes a `File`
- stdin mode passes `io::stdin().lock()`

### `create` mode

- `ricotta create temperature city temp`
  - registers collection `temperature` with ordered headers `city,temp`

- `ricotta --repo .risotto-dev create temperature city temp`
  - runs the same operation in a custom repository root

Calls `ricotta::create_collection_in_repo`.

### `get` modes

- **stdout mode**: `ricotta get temperature/london`
- **file mode**: `ricotta get temperature/london --file out.csv`

Both call `ricotta::get_to_writer_in_repo`, passing either stdout or a file.

### `list` modes

- `ricotta list`
  - lists collection names via `ricotta::list_collections_in_repo`
- `ricotta list temperature`
  - lists dataset names in `temperature` via
    `ricotta::list_datasets_in_collection_in_repo`

### Error behavior

All failures bubble through `anyhow::Result` and are printed by the
runtime with context.

### Logging behavior

- `--debug` initializes `env_logger` at `debug` level.
- Repository file accesses use `log::debug!`, so read/write traces are emitted
  only when debug logging is enabled.

### CLI tests

`main.rs` includes parser-focused unit tests (using `Cli::try_parse_from`) for:

- `create` with collection + headers
- `set` / `get` namespaced key and optional `--file`
- `list` without argument (collections) and with collection argument
