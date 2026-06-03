# crawk

[![Checks](https://github.com/BartoszCiesla/crawk/actions/workflows/check.yml/badge.svg)](https://github.com/BartoszCiesla/crawk/actions/workflows/check.yml)
[![crates.io](https://img.shields.io/crates/v/crawk.svg)](https://crates.io/crates/crawk)
[![docs.rs](https://docs.rs/crawk/badge.svg)](https://docs.rs/crawk)
[![codecov](https://codecov.io/gh/BartoszCiesla/crawk/branch/master/graph/badge.svg?token=TZFVBYEOZJ)](https://codecov.io/gh/BartoszCiesla/crawk)
[![License](https://img.shields.io/crates/l/crawk.svg)](LICENSE)

Dependency crawler for Rust. It crawls so you don't have to untangle

## Features

- **Module listing**: See which modules are defined in a crate.
- **Module usage**: See which modules are used by a given module in a crate.
- **Dependency graph**: Visualize inter-module dependencies, detect cycles, find orphan modules.
- **Dependency explanation**: See why one module depends on another — lists concrete references that create the dependency edge.

## Installation

### Cargo

```sh
cargo install crawk
# or, faster with binstall:
cargo binstall crawk
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
# Analyze which internal modules a given module depends on:
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

#### Dependency graph - `deps` subcommand

```sh
# Show full inter-module dependency graph:
crawk deps

# Detect dependency cycles:
crawk deps --cycles

# Find modules with no incoming dependencies:
crawk deps --orphans

# Export as Graphviz dot format:
crawk deps --format dot

# Annotate edges with API symbol names:
crawk deps --show-apis

# Analyze a crate at a custom path:
crawk -p /path/to/my-crate deps --cycles highlight
```

#### Dependency explanation - `why` subcommand

```sh
# Explain why module 'analyzer' depends on module 'reference':
crawk why analyzer reference

# Include submodules of the source module:
crawk why parser reference -r

# Include test modules in the search:
crawk why analyzer model -t

# Group output by source submodule (useful with -r):
crawk why parser reference -r --format grouped

# Analyze a crate at a custom path:
crawk -p /path/to/my-crate why db utils
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

# Help for the 'deps' subcommand (including options specific to dependency graph)
crawk deps --help

# Help for the 'why' subcommand (including options specific to dependency explanation)
crawk why --help
```

Note: option `-h` gives short help, while `--help` provides detailed information about available options and their usage.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for details.

## Why "crawk"?

The name `crawk` playfully blends `crawl` and `awk`, capturing the tool's mission to crawl Rust code for module dependency analysis.
It sounds action-oriented, evoking exploration of crate structures.
It was created to help me understand and manage the often complex web of module dependencies in my projects.
