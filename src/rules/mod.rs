//! Architectural rule checking.
//!
//! Evaluates user-defined module dependency contracts (loaded from a
//! `crawk.toml` / `.crawk.toml` file) against the crate's
//! [`DependencyGraph`](crate::DependencyGraph). A violation is **data**
//! ([`Violation`]), never an `Err` — only operational problems (missing or
//! malformed config, unknown module in a rule) surface as
//! [`AnalysisError`](crate::AnalysisError).
//!
//! Three check categories are supported: `layers` (named layer groups, each an
//! independent total order over a subtree of the module hierarchy; groups may
//! overlap — a module that falls under several groups is checked in each),
//! `deny` (an explicit ban on edges matching a `from` -> `to` pattern pair), and
//! `deny-cycles` (a ban on dependency loops, with an [`AllowedCycle`] list that
//! grandfathers the loops a crate already has).
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
use crate::module_path::is_in_subtree;

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

/// What `crawk check --init` wrote.
///
/// This type is marked `#[non_exhaustive]`; new fields may be added without a
/// breaking change.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct InitOutcome {
    /// Path of the config file that was created.
    pub path: PathBuf,
    /// How many existing dependency loops were frozen as `[[check.allow-cycle]]`
    /// entries, so `deny-cycles` starts green.
    pub frozen_cycles: usize,
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
    ///
    /// A subtree pattern delegates to [`is_in_subtree`] — including the empty
    /// base (`*`), which denotes the crate root and therefore matches every
    /// module. Both constructors guarantee an empty base implies `subtree`.
    pub(crate) fn matches(&self, module: &str) -> bool {
        if self.subtree {
            is_in_subtree(module, &self.base)
        } else {
            module == self.base
        }
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

    /// The pattern as the user wrote it, keeping the `::*` subtree suffix.
    ///
    /// Used by `deny`, where subtree matching is opt-in (explicit `::*`) and the
    /// suffix must be quoted verbatim in diagnostics — unlike `layers`, where
    /// subtree coverage is implicit and [`display`](Self::display) hides it.
    fn pattern_display(&self) -> String {
        if self.base.is_empty() {
            "*".to_owned()
        } else if self.subtree {
            format!("{}::*", self.base)
        } else {
            self.base.clone()
        }
    }
}

/// An explicit edge ban: no module matching `from` may depend on a module
/// matching `to`. Patterns match the subtree only with an explicit `::*`.
#[derive(Debug, Clone)]
pub(crate) struct DenyRule {
    pub(crate) from: ModulePattern,
    pub(crate) to: ModulePattern,
}

impl DenyRule {
    /// Human-readable rule citation for diagnostics and violation reports.
    pub(crate) fn display(&self) -> String {
        format!(
            "deny {} -> {}",
            self.from.pattern_display(),
            self.to.pattern_display()
        )
    }
}

/// Is this loop just a module tangled with its own descendants?
///
/// A parent that re-exports a submodule while the child reaches back with
/// `use super::…` forms an SCC in nearly every Rust crate — containment, not an
/// architectural tangle. Detected as "one module of the loop is an ancestor of
/// all the others". Evaluation skips such loops by default, and scaffolding
/// leaves them out of the generated allowlist for the same reason.
fn is_parent_child_cycle(modules: &BTreeSet<String>) -> bool {
    modules
        .iter()
        .any(|root| modules.iter().all(|module| is_in_subtree(module, root)))
}

/// A grandfathered dependency loop: one cycle that `deny-cycles` lets through
/// until it is untangled.
///
/// Entries name exact modules, never patterns — a `foo::*` would also wave
/// through loops that do not exist yet, and the point of the list is to ratchet.
#[derive(Debug, Clone)]
pub(crate) struct AllowedCycle {
    /// Modules of the known loop.
    pub(crate) modules: BTreeSet<String>,
    /// Why the loop is tolerated, quoted back when the entry goes stale.
    pub(crate) reason: Option<String>,
}

impl AllowedCycle {
    /// Human-readable citation for diagnostics: `allow-cycle [alpha, beta]`.
    pub(crate) fn display(&self) -> String {
        let list = self
            .modules
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        format!("allow-cycle [{list}]")
    }

    /// Does this entry cover a detected cycle?
    ///
    /// Subset, not equality: a loop that shrank after a partial fix stays
    /// covered, while a module joining the loop escapes the entry and is
    /// reported.
    pub(crate) fn covers(&self, cycle: &BTreeSet<String>) -> bool {
        cycle.is_subset(&self.modules)
    }
}

/// A named layer group: an independent total order over a fragment of the
/// module tree. `order[0]` is the highest layer.
#[derive(Debug, Clone)]
pub(crate) struct LayerRule {
    pub(crate) name: String,
    pub(crate) order: Vec<ModulePattern>,
    /// Whether a dependency between two modules in the *same* layer of this
    /// group is a violation. Resolved at load time from the group's own
    /// `deny-same-layer`, falling back to the `[check]`-level default.
    pub(crate) deny_same_layer: bool,
}

/// Where a module sits: which layer group, and its index within that group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayerPos {
    pub(crate) group: usize,
    pub(crate) index: usize,
}

/// A validated set of architectural rules, ready to evaluate.
///
/// Construct via [`RuleSet::load`]. Holds `layers` groups, `deny` rules, and the
/// cycle policy (`deny_cycles` plus its allowlist).
#[derive(Debug, Clone, Default)]
pub(crate) struct RuleSet {
    layers: Vec<LayerRule>,
    deny: Vec<DenyRule>,
    /// Loops that pass despite `deny_cycles`.
    allow_cycles: Vec<AllowedCycle>,
    strict_layers: bool,
    deny_cycles: bool,
    /// Report loops between a module and its own descendants too. Off by
    /// default: a parent re-exporting a submodule that reaches back with
    /// `use super::…` is containment, not an architectural tangle.
    deny_parent_child_cycles: bool,
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
    /// A `deny` rule matched the edge (explicitly banned dependency).
    ///
    /// Declared before `Layer` so `DENY` rows sort first in reports (the
    /// derived `Ord` on [`Violation`] compares `kind` first).
    Deny,
    /// A `layers` ordering was broken (dependency points "upward").
    Layer,
    /// The edge takes part in a dependency cycle banned by `deny-cycles`.
    ///
    /// Declared last so `CYCLE` rows sort after `DENY` and `LAYER`.
    Cycle,
}

impl Display for ViolationKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Deny => f.write_str("DENY"),
            Self::Layer => f.write_str("LAYER"),
            Self::Cycle => f.write_str("CYCLE"),
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

/// The result of evaluating architectural rules against a dependency graph.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_pattern_only() {
        let pattern = ModulePattern::parse("format");
        assert!(pattern.matches("format"));
        assert!(!pattern.matches("format::use_cmd"));
    }

    #[test]
    fn matches_subtree_covers_base_and_descendants() {
        let pattern = ModulePattern::parse("format::*");
        assert!(pattern.matches("format"));
        assert!(pattern.matches("format::use_cmd"));
        assert!(pattern.matches("format::use_cmd::inner"));
    }

    #[test]
    fn matches_respects_segment_boundary() {
        for pattern in [
            ModulePattern::parse("format"),
            ModulePattern::parse("format::*"),
            ModulePattern::parse_subtree("format"),
        ] {
            assert!(!pattern.matches("format_helper"));
        }
    }

    #[test]
    fn parse_subtree_upgrades_a_bare_name() {
        let pattern = ModulePattern::parse_subtree("format");
        assert!(pattern.matches("format"));
        assert!(pattern.matches("format::use_cmd"));
    }

    fn allowed(modules: &[&str]) -> AllowedCycle {
        AllowedCycle {
            modules: modules.iter().map(ToString::to_string).collect(),
            reason: None,
        }
    }

    fn cycle_of(modules: &[&str]) -> BTreeSet<String> {
        modules.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn allowed_cycle_covers_subset_and_exact_match() {
        let entry = allowed(&["alpha", "beta", "gamma"]);
        assert!(entry.covers(&cycle_of(&["alpha", "beta", "gamma"])));
        assert!(entry.covers(&cycle_of(&["alpha", "beta"])));
    }

    #[test]
    fn allowed_cycle_does_not_cover_a_grown_loop() {
        let entry = allowed(&["alpha", "beta"]);
        assert!(!entry.covers(&cycle_of(&["alpha", "beta", "gamma"])));
        assert!(!entry.covers(&cycle_of(&["delta", "epsilon"])));
    }

    #[test]
    fn allowed_cycle_display_lists_modules_alphabetically() {
        let entry = allowed(&["gamma", "alpha"]);
        assert_eq!(entry.display(), "allow-cycle [alpha, gamma]");
    }

    #[test]
    fn star_matches_every_module_including_the_root() {
        for text in ["*", "::*"] {
            let pattern = ModulePattern::parse(text);
            assert!(pattern.matches(""));
            assert!(pattern.matches("format"));
            assert!(pattern.matches("format::use_cmd"));
        }
    }
}
