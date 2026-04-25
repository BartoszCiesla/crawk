//! crawk - Rust module dependency analyzer
//!
//! This library provides tools for analyzing module dependencies in Rust codebases.
//! It parses Rust source code to identify all internal crate references including
//! `use` statements, type annotations, trait bounds, struct literals, and macro invocations.
//!
//! # Quick Start
//!
//! ```no_run
//! # use crawk::{Analyzer, AnalysisOptions};
//! # use std::path::Path;
//! # fn main() -> Result<(), crawk::AnalysisError> {
//! let mut analyzer = Analyzer::new(Path::new("/path/to/crate"))?;
//! let options = AnalysisOptions::default();
//! let result = analyzer.analyze_module("utils::parser", &options)?;
//!
//! for (module, refs) in result.dependencies() {
//!     println!("{module}");
//!     for reference in refs {
//!         println!("  {reference}");
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Key Concepts
//!
//! ## Module path
//!
//! A **module path** is the `::` separated path to a module within the crate, mirroring
//! the file layout under `src/`. For example:
//!
//! | File                       | Module path         |
//! |----------------------------|---------------------|
//! | `src/lib.rs`               | *(empty string)*    |
//! | `src/utils.rs`             | `"utils"`           |
//! | `src/utils/parser.rs`      | `"utils::parser"`   |
//! | `src/utils/parser/lexer.rs`| `"utils::parser::lexer"` |
//!
//! Inline modules (declared with `mod foo { … }` inside a file) are addressed by their
//! full path too, e.g. `"utils::parser::tests"` for a `mod tests` block inside
//! `src/utils/parser.rs`.
//!
//! ## Internal crate references
//!
//! crawk reports **internal crate references** — paths that point to items defined inside
//! the analyzed crate itself, not to `std` or external dependencies. A path is considered
//! internal if it starts with `crate::`, `self::`, `super::`, or the crate's own name.
//! All such paths are normalized to their canonical form before being returned.
//!
//! ## Relationship between `Analyzer::new` and `analyze_module`
//!
//! [`Analyzer::new`](Analyzer::new) takes the **crate root directory** (the one containing
//! `Cargo.toml`) and initializes internal state. [`Analyzer::analyze_module`](Analyzer::analyze_module)
//! then takes a **module path** (see above) relative to that crate and returns its dependencies.
//! The same `Analyzer` instance can be reused to analyze multiple modules; it caches parsed
//! files internally to avoid redundant I/O.
//!
//! # MSRV
//!
//! Requires Rust **1.85** or later (`edition = "2024"`).
//!
//! # Features
//!
//! - **Comprehensive dependency detection**: Captures not just `use` statements but also
//!   type annotations, trait bounds, struct patterns/literals, impl blocks, and macro invocations
//! - **Path expansion**: Resolves `self::` and `super::` to absolute `crate::` paths
//! - **Glob expansion**: Optionally expands `use crate::foo::*` to explicit items
//! - **Test module filtering**: Optionally include or exclude `#[cfg(test)]` modules
//! - **Depth limiting**: Truncate [`TypeReference`] paths via [`TypeReference::truncate_to_depth`]

mod analyzer;
mod cache;
mod constants;
mod discover;
mod error;
mod model;
mod parser;
mod reference;
mod resolve;
mod utils;
pub mod version;

pub use crate::analyzer::Analyzer;
pub use crate::discover::{ModuleInfo, ModuleVisibility};
pub use crate::error::AnalysisError;
pub use crate::model::{AnalysisOptions, AnalysisResult};
pub use crate::reference::{GroupItem, PathPrefix, PathSuffix, TypeReference};
