use std::collections::{BTreeMap, BTreeSet, HashSet};

use crawk::{AnalysisResult, PathPrefix};

/// A directed module dependency edge: `(source_module, target_module)`.
pub(crate) type Edge = (String, String);

/// Truncate a `::` separated module path to at most `depth` segments.
///
/// Returns the path unchanged when `depth` is `None` or when the path already
/// has fewer segments than `depth`.
fn truncate_module_path(path: &str, depth: Option<usize>) -> String {
    depth.map_or_else(
        || path.to_owned(),
        |n| path.split("::").take(n).collect::<Vec<_>>().join("::"),
    )
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
fn find_module_target<'a>(
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

/// Build a sorted, deduplicated set of module-to-module dependency edges.
///
/// Each edge `(A, B)` means module `A` references an item whose owning module
/// is `B`. Two kinds of references are tracked:
///
/// - `crate::` prefixed — intra-target references resolved via `known_modules`
/// - `<package>::` prefixed — cross-target references from a binary/test to the
///   lib target; the package-name prefix is stripped and the rest is resolved
///   against `known_modules`, falling back to `"lib"` when no specific module
///   matches (e.g. `crawk::Analyzer` re-exported at crate root → `"lib"`)
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
/// duplicate edges are removed automatically via [`BTreeSet`].
///
/// # Parameters
///
/// - `result` — analysis result from [`crawk::Analyzer::analyze_module`]
///   (called with `recursive = true`, `expand_groups = true`)
/// - `depth` — optional segment limit applied to both sides
/// - `known_modules` — set of all module paths in the crate; used to resolve
///   type references to their owning module
/// - `package_name` — crate package name (e.g. `"crawk"`); when set, refs
///   prefixed with this name are treated as cross-target lib dependencies
pub(crate) fn build_edges(
    result: &AnalysisResult,
    depth: Option<usize>,
    known_modules: &HashSet<String>,
    package_name: Option<&str>,
) -> BTreeSet<Edge> {
    let mut edges = BTreeSet::new();

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
            let segments = reference.segments();
            if segments.is_empty() {
                continue;
            }

            let module_path: Option<&str> = match reference.prefix() {
                // Intra-target: crate:: references resolved directly.
                PathPrefix::Crate => find_module_target(segments, known_modules),

                // Cross-target: <package>::Foo references from binaries/tests to lib.
                PathPrefix::None => {
                    let is_pkg_ref = package_name
                        .is_some_and(|pkg| segments.first().map(String::as_str) == Some(pkg));
                    if !is_pkg_ref {
                        continue;
                    }
                    // Strip the package-name prefix and resolve the remainder.
                    // Fall back to "lib" when no specific module matches (e.g.
                    // items re-exported directly from the crate root).
                    Some(find_module_target(&segments[1..], known_modules).unwrap_or("lib"))
                }

                _ => continue,
            };

            let Some(module_path) = module_path else {
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

            edges.insert((source.clone(), target));
        }
    }

    edges
}

/// Render dependency edges as a plain sorted list.
///
/// Each line has the format `source -> target`. Returns an empty string when
/// there are no edges.
pub(crate) fn render_plain(edges: &BTreeSet<Edge>) -> String {
    if edges.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (source, target) in edges {
        out.push_str(source);
        out.push_str(" -> ");
        out.push_str(target);
        out.push('\n');
    }
    out
}

/// Render dependency edges grouped by source module.
///
/// Each source module appears as a header `name (N)` where N is the fan-out
/// count, followed by its targets indented with `  -> `. Groups are separated
/// by blank lines and sorted alphabetically. Returns an empty string when
/// there are no edges.
pub(crate) fn render_grouped(edges: &BTreeSet<Edge>) -> String {
    if edges.is_empty() {
        return String::new();
    }

    // Collect targets per source. BTreeSet iteration is already sorted by
    // (source, target), so insertion order gives sorted targets within each group.
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (source, target) in edges {
        groups
            .entry(source.as_str())
            .or_default()
            .push(target.as_str());
    }

    let mut out = String::new();
    let mut first = true;
    for (source, targets) in &groups {
        if !first {
            out.push('\n');
        }
        first = false;
        out.push_str(source);
        out.push_str(" (");
        out.push_str(&targets.len().to_string());
        out.push_str(")\n");
        for target in targets {
            out.push_str("  -> ");
            out.push_str(target);
            out.push('\n');
        }
    }
    out
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

    // Helper: build a BTreeSet<Edge> directly for render_plain tests.
    fn edges(pairs: &[(&str, &str)]) -> BTreeSet<Edge> {
        pairs
            .iter()
            .map(|(s, t)| (s.to_string(), t.to_string()))
            .collect()
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

    // ---- render_grouped -----------------------------------------------------

    #[test]
    fn render_grouped_empty() {
        assert_eq!(render_grouped(&BTreeSet::new()), "");
    }

    #[test]
    fn render_grouped_single_source() {
        let e = edges(&[("parser", "constants"), ("parser", "reference")]);
        assert_eq!(
            render_grouped(&e),
            "parser (2)\n  -> constants\n  -> reference\n"
        );
    }

    #[test]
    fn render_grouped_multiple_sources_separated_by_blank_line() {
        let e = edges(&[("analyzer", "cache"), ("parser", "reference")]);
        assert_eq!(
            render_grouped(&e),
            "analyzer (1)\n  -> cache\n\nparser (1)\n  -> reference\n"
        );
    }

    #[test]
    fn render_grouped_targets_sorted_alphabetically() {
        let e = edges(&[("cli", "model"), ("cli", "analyzer"), ("cli", "format")]);
        assert_eq!(
            render_grouped(&e),
            "cli (3)\n  -> analyzer\n  -> format\n  -> model\n"
        );
    }

    #[test]
    fn render_grouped_ends_with_newline() {
        let e = edges(&[("a", "b")]);
        assert!(render_grouped(&e).ends_with('\n'));
    }

    // ---- render_plain -------------------------------------------------------

    #[test]
    fn render_plain_empty() {
        assert_eq!(render_plain(&BTreeSet::new()), "");
    }

    #[test]
    fn render_plain_single_edge() {
        let e = edges(&[("analyzer", "cache")]);
        assert_eq!(render_plain(&e), "analyzer -> cache\n");
    }

    #[test]
    fn render_plain_sorted_alphabetically() {
        let e = edges(&[("z_mod", "target"), ("a_mod", "target")]);
        assert_eq!(render_plain(&e), "a_mod -> target\nz_mod -> target\n");
    }

    #[test]
    fn render_plain_ends_with_newline() {
        let e = edges(&[("a", "b")]);
        assert!(render_plain(&e).ends_with('\n'));
    }
}
