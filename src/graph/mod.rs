//! Dependency graph analysis for Rust crate modules.
//!
//! Build a module-level dependency graph via [`crate::Analyzer::dependency_graph`],
//! then query it for edges, cycles (Tarjan's SCC), and orphan modules.
//!
//! The main entry points are [`DependencyGraphOptions`] (configuration) and
//! [`DependencyGraph`] (results).

mod cycles;
mod edges;
mod orphans;

use std::collections::BTreeSet;

pub use cycles::Cycle;
pub use edges::{AnnotatedEdges, Edge};

pub(crate) use cycles::detect_cycles;
pub(crate) use edges::truncate_module_path;
pub(crate) use orphans::find_orphans;

pub(crate) use edges::build_edges;
pub(crate) use edges::find_module_target;

/// Options controlling dependency graph construction.
///
/// Passed to [`crate::Analyzer::dependency_graph`] to configure which modules
/// are included and how edges are built.
///
/// This struct is `#[non_exhaustive]` — construct it via [`Default::default`]
/// and set individual fields. New options may be added in future versions
/// without a breaking change.
///
/// # Examples
///
/// ```
/// use crawk::DependencyGraphOptions;
///
/// let mut opts = DependencyGraphOptions::default();
/// opts.include_tests = true;
/// opts.depth = Some(1);
/// ```
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct DependencyGraphOptions {
    /// Include `#[cfg(test)]` modules and integration test targets.
    pub include_tests: bool,
    /// Truncate module paths to at most this many `::` segments.
    /// `None` means no truncation.
    pub depth: Option<usize>,
    /// Collect API symbol names (types, functions, traits, …) per edge.
    pub show_apis: bool,
}

/// A module-level dependency graph for a Rust crate.
///
/// Encapsulates the edges and module set produced by
/// [`Analyzer::dependency_graph`](crate::Analyzer::dependency_graph).
/// Provides access to the raw edges and derived analyses (cycles, orphans).
///
/// This type cannot be constructed directly — obtain it from
/// [`Analyzer::dependency_graph`](crate::Analyzer::dependency_graph).
///
/// # Examples
///
/// ```no_run
/// # use crawk::{Analyzer, DependencyGraphOptions};
/// # use std::path::Path;
/// # fn main() -> Result<(), crawk::AnalysisError> {
/// let mut analyzer = Analyzer::new(Path::new("/path/to/crate"))?;
/// let graph = analyzer.dependency_graph(&DependencyGraphOptions::default())?;
///
/// println!("{} edges, {} modules", graph.edges().len(), graph.modules().len());
///
/// for cycle in &graph.cycles() {
///     println!("Cycle: {:?}", cycle.modules);
/// }
///
/// for orphan in &graph.orphans() {
///     println!("Orphan: {orphan}");
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    edges: AnnotatedEdges,
    modules: BTreeSet<String>,
}

impl DependencyGraph {
    /// Create a new dependency graph from pre-built edges and module set.
    pub(crate) const fn new(edges: AnnotatedEdges, modules: BTreeSet<String>) -> Self {
        Self { edges, modules }
    }

    /// All dependency edges in the graph.
    ///
    /// Each key is an [`Edge`] `(source, target)` pair where `source` depends
    /// on `target`. Values are the set of API symbol names referenced across
    /// that edge (empty when [`DependencyGraphOptions::show_apis`] was `false`).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crawk::{Analyzer, DependencyGraphOptions};
    /// # use std::path::Path;
    /// # fn main() -> Result<(), crawk::AnalysisError> {
    /// # let mut a = Analyzer::new(Path::new("."))?;
    /// # let g = a.dependency_graph(&DependencyGraphOptions::default())?;
    /// for ((source, target), apis) in g.edges() {
    ///     println!("{source} -> {target}");
    /// }
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub const fn edges(&self) -> &AnnotatedEdges {
        &self.edges
    }

    /// All module paths in the graph, depth-truncated if a depth was specified
    /// in [`DependencyGraphOptions::depth`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crawk::{Analyzer, DependencyGraphOptions};
    /// # use std::path::Path;
    /// # fn main() -> Result<(), crawk::AnalysisError> {
    /// # let mut a = Analyzer::new(Path::new("."))?;
    /// # let g = a.dependency_graph(&DependencyGraphOptions::default())?;
    /// for module in g.modules() {
    ///     println!("{module}");
    /// }
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub const fn modules(&self) -> &BTreeSet<String> {
        &self.modules
    }

    /// Detect dependency cycles in the graph.
    ///
    /// Uses Tarjan's strongly connected components algorithm to find groups
    /// of mutually-dependent modules. Only components with 2+ modules are
    /// returned, sorted by their first module name (alphabetically).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crawk::{Analyzer, DependencyGraphOptions};
    /// # use std::path::Path;
    /// # fn main() -> Result<(), crawk::AnalysisError> {
    /// # let mut a = Analyzer::new(Path::new("."))?;
    /// # let g = a.dependency_graph(&DependencyGraphOptions::default())?;
    /// let cycles = g.cycles();
    /// if cycles.is_empty() {
    ///     println!("No dependency cycles found.");
    /// } else {
    ///     for cycle in &cycles {
    ///         println!("Cycle between: {:?}", cycle.modules);
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn cycles(&self) -> Vec<Cycle> {
        detect_cycles(&self.edges)
    }

    /// Find orphan modules — modules with no incoming edges.
    ///
    /// Orphans include target entry points (lib, main) which naturally have
    /// no dependents. Returns module paths in sorted order.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crawk::{Analyzer, DependencyGraphOptions};
    /// # use std::path::Path;
    /// # fn main() -> Result<(), crawk::AnalysisError> {
    /// # let mut a = Analyzer::new(Path::new("."))?;
    /// # let g = a.dependency_graph(&DependencyGraphOptions::default())?;
    /// for orphan in &g.orphans() {
    ///     println!("Orphan: {orphan}");
    /// }
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn orphans(&self) -> Vec<String> {
        find_orphans(&self.edges, &self.modules)
    }

    /// Returns `true` if the graph has no edges.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn make_graph(pairs: &[(&str, &str)], module_names: &[&str]) -> DependencyGraph {
        let edges: AnnotatedEdges = pairs
            .iter()
            .map(|(s, t)| ((s.to_string(), t.to_string()), BTreeSet::new()))
            .collect();
        let modules: BTreeSet<String> = module_names.iter().map(|s| (*s).to_owned()).collect();
        DependencyGraph::new(edges, modules)
    }

    #[test]
    fn empty_graph() {
        let g = DependencyGraph::new(BTreeMap::new(), BTreeSet::new());
        assert!(g.is_empty());
        assert!(g.cycles().is_empty());
        assert!(g.orphans().is_empty());
        assert!(g.modules().is_empty());
    }

    #[test]
    fn edges_and_modules_accessible() {
        let g = make_graph(&[("a", "b")], &["a", "b"]);
        assert_eq!(g.edges().len(), 1);
        assert_eq!(g.modules().len(), 2);
        assert!(!g.is_empty());
    }

    #[test]
    fn cycles_detected() {
        let g = make_graph(&[("a", "b"), ("b", "a")], &["a", "b"]);
        let cycles = g.cycles();
        assert_eq!(cycles.len(), 1);
        assert!(cycles[0].modules.contains("a"));
        assert!(cycles[0].modules.contains("b"));
    }

    #[test]
    fn orphans_detected() {
        let g = make_graph(&[("a", "b")], &["a", "b", "c"]);
        let orphans = g.orphans();
        assert_eq!(orphans, vec!["a", "c"]);
    }

    #[test]
    fn no_cycles_in_dag() {
        let g = make_graph(&[("a", "b"), ("b", "c")], &["a", "b", "c"]);
        assert!(g.cycles().is_empty());
    }
}
