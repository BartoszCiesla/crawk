//! Architectural rule checking.
//!
//! Evaluates user-defined module dependency contracts (loaded from a
//! `crawk.toml` / `.crawk.toml` file) against the crate's
//! [`DependencyGraph`](crate::DependencyGraph). A violation is **data**
//! ([`Violation`]), never an `Err` — only operational problems (missing or
//! malformed config, unknown module in a rule) surface as
//! [`AnalysisError`](crate::AnalysisError).
//!
//! Check category `layers` is currently supported (named layer groups, each an
//! independent total order over a subtree of the module hierarchy). Groups may
//! overlap: a module that falls under several groups is checked in each.
//!
//! The config is **required**: a *missing* file is an operational error (so a
//! typo fails CI rather than passing silently), whereas an *empty* `[check]`
//! table is valid and yields zero rules (always clean). `crawk check --init`
//! ([`scaffold_config`]) writes a starter file from the discovered modules.

mod eval;
mod load;

use std::collections::BTreeSet;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::PathBuf;

use crate::graph::DependencyGraphOptions;

pub(crate) use eval::evaluate;
pub(crate) use load::{resolve_config_path, scaffold_config};

/// Options controlling a `check` run.
#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    /// Explicit rule-config path. `None` triggers `crawk.toml` / `.crawk.toml`
    /// discovery in the crate root.
    pub config: Option<PathBuf>,
    /// Include `#[cfg(test)]` modules and test targets in the graph.
    pub include_tests: bool,
    /// Annotate violations with the API symbols that create the offending edge.
    pub show_apis: bool,
}

impl CheckOptions {
    /// Map to [`DependencyGraphOptions`]. Depth is left at `None` so layer
    /// checking sees full-granularity module paths.
    pub(crate) const fn graph_opts(&self) -> DependencyGraphOptions {
        DependencyGraphOptions {
            include_tests: self.include_tests,
            depth: None,
            show_apis: self.show_apis,
        }
    }
}

/// A module match pattern: an exact module, or a subtree (`foo::*`).
///
/// Matching is always on `::` segment boundaries — `format` never matches
/// `format_helper`.
#[derive(Debug, Clone)]
pub(crate) struct ModulePattern {
    base: String,
    subtree: bool,
}

impl ModulePattern {
    /// Parse a pattern. A trailing `::*` (or a lone `*`) marks a subtree match.
    pub(crate) fn parse(text: &str) -> Self {
        if text == "*" {
            return Self {
                base: String::new(),
                subtree: true,
            };
        }
        text.strip_suffix("::*").map_or_else(
            || Self {
                base: text.to_owned(),
                subtree: false,
            },
            |base| Self {
                base: base.to_owned(),
                subtree: true,
            },
        )
    }

    /// Parse a pattern that always covers the subtree. Used by `layers`, where a
    /// bare module name implicitly includes all of its descendants.
    pub(crate) fn parse_subtree(text: &str) -> Self {
        let mut pattern = Self::parse(text);
        pattern.subtree = true;
        pattern
    }

    /// Segment count of the base — higher means a more specific match.
    fn specificity(&self) -> usize {
        if self.base.is_empty() {
            0
        } else {
            self.base.split("::").count()
        }
    }

    /// Does `module` fall under this pattern?
    pub(crate) fn matches(&self, module: &str) -> bool {
        if self.base.is_empty() {
            return self.subtree; // "*" matches everything
        }
        if module == self.base {
            return true;
        }
        self.subtree
            && module
                .strip_prefix(self.base.as_str())
                .is_some_and(|rest| rest.starts_with("::"))
    }

    /// Is there at least one known module this pattern could refer to?
    fn references_known(&self, modules: &BTreeSet<String>) -> bool {
        self.base.is_empty() || modules.iter().any(|m| self.matches(m))
    }

    /// The referenced module path, for diagnostics (the implicit subtree `::*`
    /// suffix is omitted — `["cli", "typo"]` reports `typo`, not `typo::*`).
    fn display(&self) -> String {
        if self.base.is_empty() {
            "*".to_owned()
        } else {
            self.base.clone()
        }
    }
}

/// A named layer group: an independent total order over a fragment of the
/// module tree. `order[0]` is the highest layer.
#[derive(Debug, Clone)]
pub(crate) struct LayerRule {
    pub(crate) name: String,
    pub(crate) order: Vec<ModulePattern>,
}

/// Where a module sits: which layer group, and its index within that group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayerPos {
    pub(crate) group: usize,
    pub(crate) index: usize,
}

/// A validated set of architectural rules, ready to evaluate.
///
/// Construct via [`RuleSet::load`]. Currently, includes only `layers` groups.
#[derive(Debug, Clone, Default)]
pub(crate) struct RuleSet {
    layers: Vec<LayerRule>,
    strict_layers: bool,
    deny_same_layer: bool,
}

impl RuleSet {
    /// All layer positions a module occupies — one per group that covers it.
    ///
    /// Groups are independent and may overlap, so a module can belong to
    /// several. Within a single group, the longest-prefix (most specific)
    /// matching pattern wins, with ties broken by the lowest index (highest
    /// layer); that yields at most one position per group.
    pub(crate) fn memberships(&self, module: &str) -> Vec<LayerPos> {
        let mut positions = Vec::new();
        for (group, layer) in self.layers.iter().enumerate() {
            // Pick the most specific matching pattern; on a specificity tie the
            // lowest index (highest layer) wins.
            let index = layer
                .order
                .iter()
                .enumerate()
                .filter(|(_, pattern)| pattern.matches(module))
                .map(|(index, pattern)| (pattern.specificity(), index))
                .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
                .map(|(_, index)| index);
            if let Some(index) = index {
                positions.push(LayerPos { group, index });
            }
        }
        positions
    }
}

/// The kind of architectural rule that was violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ViolationKind {
    /// A `layers` ordering was broken (dependency points "upward").
    Layer,
}

impl Display for ViolationKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Layer => f.write_str("LAYER"),
        }
    }
}

/// A single architectural rule violation.
///
/// This is **data**, not an error — a non-empty [`CheckReport`] maps to exit
/// code `1`, distinct from operational failures (`AnalysisError`, exit `2`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    /// Which rule type was broken.
    pub kind: ViolationKind,
    /// The dependent module (edge source).
    pub source: String,
    /// The depended-on module (edge target).
    pub target: String,
    /// Human-readable description of the broken rule, for CI logs.
    pub rule: String,
    /// API symbols that create the offending edge (empty unless `show_apis`).
    pub apis: BTreeSet<String>,
}

/// The result of evaluating a [`RuleSet`] against a dependency graph.
#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    /// All violations found, sorted for deterministic output.
    pub violations: Vec<Violation>,
}

impl CheckReport {
    /// `true` when no violations were found.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Process exit code: `0` when clean, `1` when violations exist.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        i32::from(!self.violations.is_empty())
    }
}
