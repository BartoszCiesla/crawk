//! Dependency graph analysis for Rust crate modules.
//!
//! This module provides types and functions for building and analyzing
//! module-level dependency graphs: cycle detection (Tarjan's SCC),
//! orphan detection, and edge construction from analysis results.

mod cycles;
mod edges;
mod orphans;

pub use cycles::{Cycle, detect_cycles};
pub use edges::{AnnotatedEdges, Edge, truncate_module_path};
pub use orphans::find_orphans;

#[allow(unused_imports)] // Used by Analyzer::dependency_graph() (added in next commit)
pub(crate) use edges::build_edges;
