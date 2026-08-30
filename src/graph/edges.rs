use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::model::AnalysisResult;
use crate::reference::{PathPrefix, TypeReference};

/// A directed module dependency edge: `(source, target)`.
///
/// The edge `(A, B)` means module `A` references an item defined in module `B`
/// — i.e. `A` depends on `B`.
pub type Edge = (String, String);

/// Dependency edges with optional API name annotations per edge.
///
/// Keys are `(source, target)` pairs; values are the set of symbol names
/// (types, functions, traits, macros, …) that the source references from the
/// target. The value set is empty when API collection is disabled.
pub type AnnotatedEdges = BTreeMap<Edge, BTreeSet<String>>;

/// Truncate a `::` separated module path to at most `depth` segments.
///
/// Returns the path unchanged when `depth` is `None` or when the path already
/// has fewer segments than `depth`. The result always borrows from `path`: a
/// truncated module path is a prefix of the original.
///
/// This is the module-path counterpart of
/// [`TypeReference::truncate_to_depth`], which counts **segments only** and
/// ignores the `crate::`/`self::`/`super::` prefix. Module paths in the graph
/// carry no prefix, so on that domain the two agree.
#[must_use]
pub(crate) fn truncate_module_path(path: &str, depth: Option<usize>) -> &str {
    let Some(n) = depth else { return path };
    if n == 0 {
        return "";
    }
    // `idx` is the start of the nth `::`, so it is always a char boundary and
    // `get` always yields `Some`; the fallback keeps the function total.
    path.match_indices("::")
        .nth(n - 1)
        .map_or(path, |(idx, _)| path.get(..idx).unwrap_or(path))
}

/// Resolve a TypeReference's segments to the module that contains the referenced item.
///
/// Tries segment prefixes from longest to shortest, returning the first that
/// is present in `known_modules`. This strips the trailing item name (type,
/// function, constant, …) and returns only the owning module path.
///
/// Examples (`known_modules` = {"discover", "discover::module_tree", "reference"}):
/// - `["discover", "CrateInfo"]`            → `"discover"`
/// - `["discover", "module_tree", "Node"]`  → `"discover::module_tree"`
/// - `["reference", "TypeReference"]`       → `"reference"`
///
/// Returns `None` when no prefix matches (e.g. a crate-root re-export with no
/// corresponding module path).
pub(crate) fn find_module_target<'a>(
    segments: &[String],
    known_modules: &'a HashSet<String>,
) -> Option<&'a str> {
    for len in (1..=segments.len()).rev() {
        let candidate = segments[..len].join("::");
        if let Some(m) = known_modules.get(&candidate) {
            return Some(m.as_str());
        }
    }
    None
}

/// Resolve a single reference to its owning module path in the dependency graph.
///
/// Shared resolution logic used by both [`build_edges`] (the `deps` command) and
/// [`crate::Analyzer::explain_dependency`] (the `why` command) so the two agree
/// on how a reference maps to a target module. Two reference kinds resolve:
///
/// - `crate::` prefixed — intra-target refs resolved via `known_modules`,
///   falling back to `"lib"` for crate-root re-exports
/// - `<package>::` prefixed — cross-target refs from a binary/test to the lib
///   target; the package-name prefix is stripped and the rest resolved,
///   falling back to `"lib"`
///
/// Returns `(module_path, api_segments)` where `api_segments` are the trailing
/// segments after the module path, or `None` when the reference is neither an
/// intra-crate nor a matching package-name dependency.
pub(crate) fn resolve_reference_target<'a>(
    reference: &'a TypeReference,
    known_modules: &'a HashSet<String>,
    package_name: Option<&str>,
) -> Option<(&'a str, &'a [String])> {
    let segments = reference.segments();
    if segments.is_empty() {
        return None;
    }

    match reference.prefix() {
        // Intra-target: crate:: references resolved directly, falling back to
        // "lib" for crate-root re-exports (no module in the path matches,
        // e.g. `crate::Widget` re-exported in lib.rs).
        PathPrefix::Crate => Some(
            find_module_target(segments, known_modules).map_or(("lib", segments), |m| {
                (m, &segments[m.split("::").count()..])
            }),
        ),

        // Cross-target: <package>::Foo references from binaries/tests to lib.
        PathPrefix::None => {
            let is_pkg_ref =
                package_name.is_some_and(|pkg| segments.first().map(String::as_str) == Some(pkg));
            if !is_pkg_ref {
                return None;
            }
            let rest = &segments[1..];
            Some(
                find_module_target(rest, known_modules)
                    .map_or(("lib", rest), |m| (m, &rest[m.split("::").count()..])),
            )
        }

        _ => None,
    }
}

/// Build a sorted, deduplicated map of module-to-module dependency edges.
///
/// Each edge `(A, B)` means module `A` references an item whose owning module
/// is `B`. Two kinds of references are tracked:
///
/// - `crate::` prefixed — intra-target references resolved via `known_modules`
/// - `<package>::` prefixed — cross-target references from a binary/test to the
///   lib target; the package-name prefix is stripped and the rest is resolved
///   against `known_modules`
///
/// Both kinds fall back to `"lib"` when no specific module matches (e.g.
/// `crate::Analyzer`/`crawk::Analyzer` re-exported at crate root → `"lib"`),
/// so a re-export never silently drops the edge.
///
/// The target is resolved by finding the longest segment prefix present in
/// `known_modules`, stripping trailing item names (types, functions, …) so that
/// the edge always points at a real module, not an item inside one.
///
/// # Depth semantics
///
/// When `depth` is `Some(n)`, **both** the source and the resolved target module
/// paths are truncated to at most `n` `::` separated segments:
///
/// - `depth = 1` — top-level modules only (`parser::visitor` → `parser`)
/// - `depth = 2` — top-level and one nesting level (`discover::module_tree` kept)
/// - `depth = None` — full module path, no truncation
///
/// After truncation, self-loops (source == target after truncation) and
/// duplicate edges are removed automatically via [`BTreeMap`].
///
/// # API names
///
/// When `show_apis` is `true`, each edge carries the set of symbol names
/// (types, functions, traits, …) that the source references from the target.
/// When `false`, API sets are left empty.
///
/// # Parameters
///
/// - `result` — analysis result from [`crate::Analyzer::analyze_module`]
///   (called with `recursive = true`, `expand_groups = true`)
/// - `depth` — optional segment limit applied to both sides
/// - `known_modules` — set of all module paths in the crate; used to resolve
///   type references to their owning module
/// - `package_name` — crate package name (e.g. `"crawk"`); when set, refs
///   prefixed with this name are treated as cross-target lib dependencies
/// - `show_apis` — collect API symbol names per edge
pub(crate) fn build_edges(
    result: &AnalysisResult,
    depth: Option<usize>,
    known_modules: &HashSet<String>,
    package_name: Option<&str>,
    show_apis: bool,
) -> AnnotatedEdges {
    let mut edges: AnnotatedEdges = BTreeMap::new();

    for (source_key, refs) in result.dependencies() {
        // An empty key represents the root module (e.g. "lib" when analysing from lib).
        let source_full = if source_key.is_empty() {
            result.module_path()
        } else {
            source_key.as_str()
        };

        let source = truncate_module_path(source_full, depth);
        if source.is_empty() {
            continue;
        }

        for reference in refs {
            let Some((module_path, api_segments)) =
                resolve_reference_target(reference, known_modules, package_name)
            else {
                continue;
            };
            let target = truncate_module_path(module_path, depth);
            if target.is_empty() {
                continue;
            }

            // Drop self-loops produced by depth truncation.
            if source == target {
                continue;
            }

            let apis = edges
                .entry((source.to_owned(), target.to_owned()))
                .or_default();
            if show_apis && !api_segments.is_empty() {
                apis.insert(api_segments.join("::"));
            }
        }
    }

    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(ToString::to_string).collect()
    }

    fn segments(names: &[&str]) -> Vec<String> {
        names.iter().map(ToString::to_string).collect()
    }

    // ---- truncate_module_path -----------------------------------------------

    #[test]
    fn truncate_module_path_none_returns_full() {
        assert_eq!(
            truncate_module_path("parser::visitor", None),
            "parser::visitor"
        );
    }

    #[test]
    fn truncate_module_path_depth_1() {
        assert_eq!(truncate_module_path("parser::visitor", Some(1)), "parser");
    }

    #[test]
    fn truncate_module_path_depth_equals_length() {
        assert_eq!(
            truncate_module_path("parser::visitor", Some(2)),
            "parser::visitor"
        );
    }

    #[test]
    fn truncate_module_path_depth_exceeds_length() {
        assert_eq!(truncate_module_path("parser", Some(5)), "parser");
    }

    #[test]
    fn truncate_module_path_depth_zero_yields_empty() {
        assert_eq!(truncate_module_path("parser::visitor", Some(0)), "");
    }

    #[test]
    fn truncate_module_path_empty_input() {
        assert_eq!(truncate_module_path("", Some(2)), "");
        assert_eq!(truncate_module_path("", None), "");
    }

    // Guards the switch from `split("::").take(n).join("::")` to index-based
    // slicing: both must treat a doubled separator the same way.
    #[test]
    fn truncate_module_path_repeated_separator_matches_split_semantics() {
        for n in 0..=4 {
            let expected = "a::::b".split("::").take(n).collect::<Vec<_>>().join("::");
            assert_eq!(truncate_module_path("a::::b", Some(n)), expected, "n = {n}");
        }
    }

    #[test]
    fn truncate_module_path_returns_slice_of_input() {
        let path = String::from("parser::visitor::inner");
        let truncated = truncate_module_path(&path, Some(2));
        assert_eq!(truncated, "parser::visitor");
        // Borrowed, not rebuilt: the slice starts inside the original buffer.
        assert!(std::ptr::eq(truncated.as_ptr(), path.as_ptr()));
    }

    // ---- find_module_target -------------------------------------------------

    #[test]
    fn finds_direct_module_match() {
        let known = module_set(&["discover", "discover::module_tree", "reference"]);
        assert_eq!(
            find_module_target(&segments(&["discover", "CrateInfo"]), &known),
            Some("discover")
        );
    }

    #[test]
    fn finds_nested_module_match() {
        let known = module_set(&["discover", "discover::module_tree"]);
        assert_eq!(
            find_module_target(&segments(&["discover", "module_tree", "Node"]), &known),
            Some("discover::module_tree")
        );
    }

    #[test]
    fn prefers_longest_prefix() {
        // Both "discover" and "discover::module_tree" are valid — pick deepest.
        let known = module_set(&["discover", "discover::module_tree"]);
        assert_eq!(
            find_module_target(&segments(&["discover", "module_tree"]), &known),
            Some("discover::module_tree")
        );
    }

    #[test]
    fn returns_none_when_no_prefix_matches() {
        let known = module_set(&["cache", "parser"]);
        assert_eq!(find_module_target(&segments(&["Analyzer"]), &known), None);
    }

    #[test]
    fn returns_none_for_empty_segments() {
        let known = module_set(&["parser"]);
        assert_eq!(find_module_target(&[], &known), None);
    }
}
