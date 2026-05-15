use std::collections::{BTreeMap, BTreeSet};

use crawk::AnnotatedEdges;

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
