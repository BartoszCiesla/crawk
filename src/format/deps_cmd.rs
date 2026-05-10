use std::collections::{BTreeMap, BTreeSet, HashSet};

use crawk::{AnalysisResult, PathPrefix};

/// A directed module dependency edge: `(source_module, target_module)`.
pub(crate) type Edge = (String, String);

/// Dependency edges with optional API name annotations per edge.
///
/// Keys are `(source, target)` pairs; values are the set of symbol names
/// (types, functions, traits, macros, …) that the source references from the
/// target. The value set is empty when API collection is disabled.
pub(crate) type AnnotatedEdges = BTreeMap<Edge, BTreeSet<String>>;

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

/// Build a sorted, deduplicated map of module-to-module dependency edges.
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
/// - `result` — analysis result from [`crawk::Analyzer::analyze_module`]
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
            let segments = reference.segments();
            if segments.is_empty() {
                continue;
            }

            // Resolve the reference to (module_path, api_segments) where
            // api_segments are the trailing segments after the module path.
            let resolved: Option<(&str, &[String])> = match reference.prefix() {
                // Intra-target: crate:: references resolved directly.
                PathPrefix::Crate => find_module_target(segments, known_modules)
                    .map(|m| (m, &segments[m.split("::").count()..])),

                // Cross-target: <package>::Foo references from binaries/tests to lib.
                PathPrefix::None => {
                    let is_pkg_ref = package_name
                        .is_some_and(|pkg| segments.first().map(String::as_str) == Some(pkg));
                    if !is_pkg_ref {
                        continue;
                    }
                    let rest = &segments[1..];
                    Some(
                        find_module_target(rest, known_modules)
                            .map_or(("lib", rest), |m| (m, &rest[m.split("::").count()..])),
                    )
                }

                _ => continue,
            };

            let Some((module_path, api_segments)) = resolved else {
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

            let apis = edges.entry((source.clone(), target)).or_default();
            if show_apis && !api_segments.is_empty() {
                apis.insert(api_segments.join("::"));
            }
        }
    }

    edges
}

/// Format an API annotation suffix for a single edge.
///
/// Returns ` [Symbol1, Symbol2]` when APIs are present, empty string otherwise.
fn format_api_suffix(apis: &BTreeSet<String>) -> String {
    if apis.is_empty() {
        return String::new();
    }
    let items: Vec<&str> = apis.iter().map(String::as_str).collect();
    format!(" [{}]", items.join(", "))
}

/// Render dependency edges as a plain sorted list.
///
/// Each line has the format `source -> target` (or `source -> target [API1, API2]`
/// when API annotations are present). Returns an empty string when there are no edges.
pub(crate) fn render_plain(edges: &AnnotatedEdges) -> String {
    if edges.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for ((source, target), apis) in edges {
        out.push_str(source);
        out.push_str(" -> ");
        out.push_str(target);
        out.push_str(&format_api_suffix(apis));
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
pub(crate) fn render_grouped(edges: &AnnotatedEdges) -> String {
    if edges.is_empty() {
        return String::new();
    }

    // Collect targets per source. BTreeMap iteration is already sorted by
    // (source, target), so insertion order gives sorted targets within each group.
    let mut groups: BTreeMap<&str, Vec<(&str, &BTreeSet<String>)>> = BTreeMap::new();
    for ((source, target), apis) in edges {
        groups
            .entry(source.as_str())
            .or_default()
            .push((target.as_str(), apis));
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
        for (target, apis) in targets {
            out.push_str("  -> ");
            out.push_str(target);
            out.push_str(&format_api_suffix(apis));
            out.push('\n');
        }
    }
    out
}

/// Render dependency edges as a Graphviz DOT digraph.
///
/// Always produces a valid DOT document — even when there are no edges the
/// graph header and attributes are emitted so the output can be piped
/// directly into `dot` or `xdot` without extra checks.
///
/// Module names containing `::` are quoted as DOT string literals so that
/// Graphviz parses them correctly. Nodes are listed explicitly before edges
/// to give renderers a stable ordering.
///
/// When API annotations are present, edges carry a `label` attribute showing
/// the symbol names.
pub(crate) fn render_dot(edges: &AnnotatedEdges) -> String {
    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=rounded, fontname=\"monospace\", fontsize=10];\n");
    out.push_str("    edge [color=\"#444444\"];\n");

    if !edges.is_empty() {
        // Collect unique nodes; BTreeSet gives deterministic sort order.
        let mut nodes: BTreeSet<&str> = BTreeSet::new();
        for (source, target) in edges.keys() {
            nodes.insert(source.as_str());
            nodes.insert(target.as_str());
        }

        out.push('\n');
        for node in &nodes {
            out.push_str("    \"");
            out.push_str(node);
            out.push_str("\";\n");
        }

        out.push('\n');
        for ((source, target), apis) in edges {
            out.push_str("    \"");
            out.push_str(source);
            out.push_str("\" -> \"");
            out.push_str(target);
            if apis.is_empty() {
                out.push_str("\";\n");
            } else {
                let items: Vec<&str> = apis.iter().map(String::as_str).collect();
                out.push_str("\" [label=\"");
                out.push_str(&items.join(", "));
                out.push_str("\", fontcolor=\"#c96a00\"];\n");
            }
        }
    }

    out.push_str("}\n");
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

    /// Build an AnnotatedEdges map without API annotations.
    fn edges(pairs: &[(&str, &str)]) -> AnnotatedEdges {
        pairs
            .iter()
            .map(|(s, t)| ((s.to_string(), t.to_string()), BTreeSet::new()))
            .collect()
    }

    /// Build an AnnotatedEdges map with API annotations.
    fn edges_with_apis(pairs: &[(&str, &str, &[&str])]) -> AnnotatedEdges {
        pairs
            .iter()
            .map(|(s, t, apis)| {
                let api_set: BTreeSet<String> = apis.iter().map(ToString::to_string).collect();
                ((s.to_string(), t.to_string()), api_set)
            })
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
        assert_eq!(render_grouped(&BTreeMap::new()), "");
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

    #[test]
    fn render_grouped_with_apis() {
        let e = edges_with_apis(&[
            ("analyzer", "cache", &["ParseCache"]),
            ("analyzer", "reference", &["PathPrefix", "TypeReference"]),
        ]);
        assert_eq!(
            render_grouped(&e),
            "analyzer (2)\n  -> cache [ParseCache]\n  -> reference [PathPrefix, TypeReference]\n"
        );
    }

    // ---- render_dot ---------------------------------------------------------

    #[test]
    fn render_dot_empty_edges_produces_valid_dot() {
        let out = render_dot(&BTreeMap::new());
        assert!(out.starts_with("digraph dependencies {"));
        assert!(out.ends_with("}\n"));
        // No node/edge lines in empty graph
        assert!(!out.contains("\" -> \""));
    }

    #[test]
    fn render_dot_single_edge() {
        let e = edges(&[("parser", "reference")]);
        let out = render_dot(&e);
        assert!(out.contains("\"parser\";"));
        assert!(out.contains("\"reference\";"));
        assert!(out.contains("\"parser\" -> \"reference\";"));
    }

    #[test]
    fn render_dot_nodes_listed_before_edges() {
        let e = edges(&[("cli", "model")]);
        let out = render_dot(&e);
        let node_pos = out.find("\"cli\";").unwrap();
        let edge_pos = out.find("\"cli\" -> \"model\";").unwrap();
        assert!(node_pos < edge_pos);
    }

    #[test]
    fn render_dot_colons_quoted_correctly() {
        let e = edges(&[("parser::visitor", "discover::module_tree")]);
        let out = render_dot(&e);
        assert!(out.contains("\"parser::visitor\";"));
        assert!(out.contains("\"discover::module_tree\";"));
        assert!(out.contains("\"parser::visitor\" -> \"discover::module_tree\";"));
    }

    #[test]
    fn render_dot_ends_with_newline() {
        let e = edges(&[("a", "b")]);
        assert!(render_dot(&e).ends_with('\n'));
    }

    #[test]
    fn render_dot_with_apis() {
        let e = edges_with_apis(&[("parser", "reference", &["PathPrefix", "TypeReference"])]);
        let out = render_dot(&e);
        assert!(out.contains("\"parser\" -> \"reference\" [label=\"PathPrefix, TypeReference\", fontcolor=\"#c96a00\"];"));
    }

    #[test]
    fn render_dot_without_apis_no_label() {
        let e = edges(&[("parser", "reference")]);
        let out = render_dot(&e);
        assert!(!out.contains("label="));
        assert!(out.contains("\"parser\" -> \"reference\";"));
    }

    // ---- render_plain -------------------------------------------------------

    #[test]
    fn render_plain_empty() {
        assert_eq!(render_plain(&BTreeMap::new()), "");
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

    #[test]
    fn render_plain_with_apis() {
        let e = edges_with_apis(&[
            ("analyzer", "cache", &["ParseCache"]),
            ("analyzer", "reference", &["PathPrefix", "TypeReference"]),
        ]);
        assert_eq!(
            render_plain(&e),
            "analyzer -> cache [ParseCache]\nanalyzer -> reference [PathPrefix, TypeReference]\n"
        );
    }
}
