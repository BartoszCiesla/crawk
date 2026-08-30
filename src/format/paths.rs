use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crawk::Edge;

/// Render paths as a plain list: one `a -> b -> c` line per path.
///
/// `paths` is expected to come from
/// [`ShortestPaths::truncated`](crawk::ShortestPaths::truncated), which applies
/// `--depth` and dedups.
#[must_use]
pub(crate) fn render_paths_plain(paths: &[Vec<String>]) -> String {
    let mut out = String::new();
    for path in paths {
        let _ = writeln!(out, "{}", path.join(" -> "));
    }
    out
}

/// Render paths grouped by hop count under a `length N:` header.
///
/// `paths` is expected to come from
/// [`ShortestPaths::truncated`](crawk::ShortestPaths::truncated), which applies
/// `--depth` and dedups.
#[must_use]
pub(crate) fn render_paths_grouped(paths: &[Vec<String>]) -> String {
    let mut groups: BTreeMap<usize, Vec<&Vec<String>>> = BTreeMap::new();
    for path in paths {
        let len = path.len().saturating_sub(1);
        groups.entry(len).or_default().push(path);
    }

    let mut out = String::new();
    let mut first = true;
    for (len, group) in &groups {
        if !first {
            out.push('\n');
        }
        first = false;
        let _ = writeln!(out, "length {len}:");
        for path in group {
            let _ = writeln!(out, "  {}", path.join(" -> "));
        }
    }
    out
}

/// Render the full dependency graph in DOT with path edges highlighted in red.
///
/// Both inputs are expected to be depth-resolved already — `edges` from
/// [`DependencyGraph::truncated_edges`](crawk::DependencyGraph::truncated_edges)
/// and `paths` from
/// [`ShortestPaths::truncated`](crawk::ShortestPaths::truncated) — so that
/// nodes and edges agree. Edges that appear on any shortest path get
/// `color=red, style=bold, penwidth=2.0`. Returns an empty string when `paths`
/// is empty (no path found).
#[must_use]
pub(crate) fn render_paths_dot(edges: &BTreeSet<Edge>, paths: &[Vec<String>]) -> String {
    if paths.is_empty() {
        return String::new();
    }

    let mut path_edge_set: BTreeSet<(&str, &str)> = BTreeSet::new();
    for path in paths {
        for w in path.windows(2) {
            path_edge_set.insert((w[0].as_str(), w[1].as_str()));
        }
    }

    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=rounded, fontname=\"monospace\", fontsize=10];\n");
    out.push_str("    edge [color=\"#444444\"];\n");

    if !edges.is_empty() {
        let mut nodes: BTreeSet<&str> = BTreeSet::new();
        for (source, target) in edges {
            nodes.insert(source.as_str());
            nodes.insert(target.as_str());
        }

        out.push('\n');
        for node in &nodes {
            let _ = writeln!(out, "    \"{node}\";");
        }

        out.push('\n');
        for (source, target) in edges {
            let on_path = path_edge_set.contains(&(source.as_str(), target.as_str()));
            let _ = write!(out, "    \"{source}\" -> \"{target}\"");
            if on_path {
                out.push_str(" [color=red, style=bold, penwidth=2.0]");
            }
            out.push_str(";\n");
        }
    }

    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(paths: Vec<Vec<&str>>) -> Vec<Vec<String>> {
        paths
            .into_iter()
            .map(|p| p.into_iter().map(str::to_owned).collect())
            .collect()
    }

    fn edges(pairs: &[(&str, &str)]) -> BTreeSet<Edge> {
        pairs
            .iter()
            .map(|(s, t)| ((*s).to_owned(), (*t).to_owned()))
            .collect()
    }

    // ---- render_paths_plain --------------------------------------------------

    #[test]
    fn plain_empty_returns_empty() {
        assert_eq!(render_paths_plain(&[]), "");
    }

    #[test]
    fn plain_single_path() {
        let p = paths(vec![vec!["a", "b", "c"]]);
        assert_eq!(render_paths_plain(&p), "a -> b -> c\n");
    }

    #[test]
    fn plain_two_paths_sorted() {
        let p = paths(vec![vec!["lib", "a", "leaf"], vec!["lib", "b", "leaf"]]);
        let out = render_paths_plain(&p);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "lib -> a -> leaf");
        assert_eq!(lines[1], "lib -> b -> leaf");
    }

    #[test]
    fn plain_source_equals_target() {
        let p = paths(vec![vec!["a"]]);
        assert_eq!(render_paths_plain(&p), "a\n");
    }

    // ---- render_paths_grouped ------------------------------------------------

    #[test]
    fn grouped_empty_returns_empty() {
        assert_eq!(render_paths_grouped(&[]), "");
    }

    #[test]
    fn grouped_has_length_header() {
        let p = paths(vec![vec!["a", "b", "c"]]);
        let out = render_paths_grouped(&p);
        assert!(out.contains("length 2:"));
        assert!(out.contains("  a -> b -> c"));
    }

    #[test]
    fn grouped_same_length_paths_one_header() {
        let p = paths(vec![vec!["lib", "a", "leaf"], vec!["lib", "b", "leaf"]]);
        let out = render_paths_grouped(&p);
        let headers: Vec<&str> = out.lines().filter(|l| l.starts_with("length")).collect();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], "length 2:");
    }

    // ---- render_paths_dot ---------------------------------------------------

    #[test]
    fn dot_empty_paths_returns_empty() {
        let e = edges(&[("a", "b")]);
        assert_eq!(render_paths_dot(&e, &[]), "");
    }

    #[test]
    fn dot_path_edge_colored_red() {
        let e = edges(&[("lib", "a"), ("lib", "b"), ("a", "leaf"), ("b", "leaf")]);
        let p = paths(vec![vec!["lib", "a", "leaf"]]);
        let out = render_paths_dot(&e, &p);
        assert!(out.contains("\"lib\" -> \"a\" [color=red, style=bold, penwidth=2.0];"));
        assert!(out.contains("\"a\" -> \"leaf\" [color=red, style=bold, penwidth=2.0];"));
        assert!(out.contains("\"lib\" -> \"b\";"));
        assert!(out.contains("\"b\" -> \"leaf\";"));
    }

    #[test]
    fn dot_starts_and_ends_correctly() {
        let e = edges(&[("a", "b")]);
        let p = paths(vec![vec!["a", "b"]]);
        let out = render_paths_dot(&e, &p);
        assert!(out.starts_with("digraph dependencies {"));
        assert!(out.ends_with("}\n"));
    }
}
