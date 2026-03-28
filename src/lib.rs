//! crawk - Rust module dependency analyzer
//!
//! This library provides tools for analyzing module dependencies in Rust codebases.
//! It parses Rust source code to identify all internal crate references including
//! `use` statements, type annotations, trait bounds, struct literals, and macro invocations.
//!
//! # Quick Start
//!
//! ```no_run
//! use crawk::{Analyzer, AnalysisOptions};
//! use std::path::Path;
//!
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
//! # Ok::<(), crawk::AnalysisError>(())
//! ```
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
pub use crate::error::{AnalysisError, Result};
pub use crate::model::{AnalysisOptions, AnalysisResult};
pub use crate::reference::{GroupItem, PathPrefix, PathSuffix, Segments, TypeReference};
