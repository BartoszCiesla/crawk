# crawk

[![Checks](https://github.com/BartoszCiesla/crawk/actions/workflows/check.yml/badge.svg)](https://github.com/BartoszCiesla/crawk/actions/workflows/check.yml)
[![crates.io](https://img.shields.io/crates/v/crawk.svg)](https://crates.io/crates/crawk)
[![docs.rs](https://docs.rs/crawk/badge.svg)](https://docs.rs/crawk)
[![License](https://img.shields.io/crates/l/crawk.svg)](LICENSE)

Dependency crawler for Rust. It crawls so you don't have to untangle

## Features

- **Module listing**: See which modules are defined in a crate.
- **Module usage**: See which modules are used by a given module in a crate.

## Installation

### Cargo

```sh
cargo install crawk
```

### Nix

```sh
# Run without installing
nix run github:BartoszCiesla/crawk

# Install into profile
nix profile install github:BartoszCiesla/crawk

# Development shell
nix develop
```

## Quick Start

### CLI

#### Module listing - `list` subcommand

```sh
# List all modules in the current crate:
crawk list

# List only a module subtree (root included):
crawk list parser

# Show only modules up to a given nesting depth:
crawk list parser --depth 2

# Filter listed modules and render them as a table:
crawk list --filter parser --format table

# Analyze a crate located at a custom path and show source files:
crawk -p /path/to/my-crate list -t -s
```

#### Module usage - `use` subcommand

```sh
# Analyze which internal modules a given module depends on
crawk use utils

# Recursively analyze all submodules of a module:
crawk use foo::bar -r

# Expand grouped imports (`a::{x, y}` → `a::x`, `a::y`) and resolve glob imports:
crawk use parser -e -G

# Limit dependency path depth and group output by source module:
crawk use server --depth 2 --format grouped

# Analyze a crate located at a custom path, including test modules:
crawk -p /path/to/my-crate use db -r -t
```

## CLI Options

Please refer to `crawk` command CLI help for a comprehensive list of options:

```sh
# General help for crawk (including global options and subcommands)
crawk --help

# Help for the 'list' subcommand (including options specific to listing modules)
crawk list --help

# Help for the 'use' subcommand (including options specific to module usage analysis)
crawk use --help
```

Note: option `-h` gives short help, while `--help` provides detailed information about available options and their usage.

## License

This project is licensed under the Apache License. See the [LICENSE](LICENSE) file for details.

## Why "crawk"?

The name `crawk` playfully blends `crawl` and `awk`, capturing the tool's mission to crawl Rust code for module dependency analysis.
It sounds action-oriented, evoking exploration of crate structures.  It sounds action-oriented, evoking exploration of crate structures.
It was created to help me understand and manage the often complex web of module dependencies in my projects.
