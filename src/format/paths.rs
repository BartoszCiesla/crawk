use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crawk::{AnnotatedEdges, ShortestPaths};

fn truncate_segment(seg: &str, depth: Option<usize>) -> String {
    depth.map_or_else(
        || seg.to_owned(),
        |n| seg.split("::").take(n).collect::<Vec<_>>().join("::"),
    )
}

/// Truncate every node in every path to `depth` segments, then dedup and sort.
///
/// When `depth` is `None`, returns a clone of `paths`.
pub(crate) fn truncate_paths_dedup(
    paths: &[Vec<String>],
    depth: Option<usize>,
) -> Vec<Vec<String>> {
    let Some(d) = depth else {
        return paths.to_vec();
    };
    let mut seen: BTreeSet<Vec<String>> = BTreeSet::new();
    for path in paths {
        let truncated: Vec<String> = path
            .iter()
            .map(|seg| truncate_segment(seg, Some(d)))
            .collect();
        seen.insert(truncated);
    }
    seen.into_iter().collect()
}

/// Render paths as a plain list: one `a -> b -> c` line per path.
#[must_use]
pub(crate) fn render_paths_plain(paths: &ShortestPaths, depth: Option<usize>) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let resolved = truncate_paths_dedup(&paths.paths, depth);
    let mut out = String::new();
    for path in &resolved {
        let _ = writeln!(out, "{}", path.join(" -> "));
    }
    out
}

/// Render paths grouped by hop count under a `length N:` header.
#[must_use]
pub(crate) fn render_paths_grouped(paths: &ShortestPaths, depth: Option<usize>) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let resolved = truncate_paths_dedup(&paths.paths, depth);

    let mut groups: BTreeMap<usize, Vec<&Vec<String>>> = BTreeMap::new();
    for path in &resolved {
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
/// All edges from `all_edges` are drawn; edges that appear on any shortest path
/// get `color=red, style=bold, penwidth=2.0`. Returns an empty string when
/// `paths` is empty (no path found).
#[must_use]
pub(crate) fn render_paths_dot(
    all_edges: &AnnotatedEdges,
    paths: &ShortestPaths,
    depth: Option<usize>,
) -> String {
    if paths.is_empty() {
        return String::new();
    }

    let resolved = truncate_paths_dedup(&paths.paths, depth);

    let mut path_edge_set: BTreeSet<(String, String)> = BTreeSet::new();
    for path in &resolved {
        for w in path.windows(2) {
            path_edge_set.insert((w[0].clone(), w[1].clone()));
        }
    }

    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=rounded, fontname=\"monospace\", fontsize=10];\n");
    out.push_str("    edge [color=\"#444444\"];\n");

    if !all_edges.is_empty() {
        let mut nodes: BTreeSet<&str> = BTreeSet::new();
        for (source, target) in all_edges.keys() {
            nodes.insert(source.as_str());
            nodes.insert(target.as_str());
        }

        out.push('\n');
        for node in &nodes {
            let _ = writeln!(out, "    \"{node}\";");
        }

        out.push('\n');
        for (source, target) in all_edges.keys() {
            let on_path = path_edge_set.contains(&(source.clone(), target.clone()));
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
    use std::collections::BTreeSet;

    use crawk::AnnotatedEdges;

    use super::*;

    fn sp(source: &str, target: &str, paths: Vec<Vec<&str>>) -> ShortestPaths {
        let owned: Vec<Vec<String>> = paths
            .into_iter()
            .map(|p| p.into_iter().map(str::to_owned).collect())
            .collect();
        ShortestPaths::new(source.to_owned(), target.to_owned(), owned)
    }

    fn edges(pairs: &[(&str, &str)]) -> AnnotatedEdges {
        pairs
            .iter()
            .map(|(s, t)| ((s.to_string(), t.to_string()), BTreeSet::new()))
            .collect()
    }

    // ---- truncate_paths_dedup -----------------------------------------------

    #[test]
    fn truncate_no_depth_clones() {
        let paths = vec![vec!["a::x".to_owned(), "b::y".to_owned()]];
        assert_eq!(truncate_paths_dedup(&paths, None), paths);
    }

    #[test]
    fn truncate_depth_1_short_nodes_unchanged() {
        let paths = vec![
            vec!["lib".to_owned(), "a".to_owned(), "leaf".to_owned()],
            vec!["lib".to_owned(), "b".to_owned(), "leaf".to_owned()],
        ];
        let result = truncate_paths_dedup(&paths, Some(1));
        // All nodes already 1 segment — both paths survive distinct
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn truncate_dedup_removes_duplicates() {
        let paths = vec![
            vec!["lib::x".to_owned(), "a::y".to_owned()],
            vec!["lib::z".to_owned(), "a::w".to_owned()],
        ];
        let result = truncate_paths_dedup(&paths, Some(1));
        // Both truncate to ["lib", "a"]
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec!["lib", "a"]);
    }

    // ---- render_paths_plain --------------------------------------------------

    #[test]
    fn plain_empty_returns_empty() {
        let s = sp("a", "b", vec![]);
        assert_eq!(render_paths_plain(&s, None), "");
    }

    #[test]
    fn plain_single_path() {
        let s = sp("a", "c", vec![vec!["a", "b", "c"]]);
        assert_eq!(render_paths_plain(&s, None), "a -> b -> c\n");
    }

    #[test]
    fn plain_two_paths_sorted() {
        let s = sp(
            "lib",
            "leaf",
            vec![vec!["lib", "a", "leaf"], vec!["lib", "b", "leaf"]],
        );
        let out = render_paths_plain(&s, None);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "lib -> a -> leaf");
        assert_eq!(lines[1], "lib -> b -> leaf");
    }

    #[test]
    fn plain_source_equals_target() {
        let s = sp("a", "a", vec![vec!["a"]]);
        assert_eq!(render_paths_plain(&s, None), "a\n");
    }

    // ---- render_paths_grouped ------------------------------------------------

    #[test]
    fn grouped_empty_returns_empty() {
        let s = sp("a", "b", vec![]);
        assert_eq!(render_paths_grouped(&s, None), "");
    }

    #[test]
    fn grouped_has_length_header() {
        let s = sp("a", "c", vec![vec!["a", "b", "c"]]);
        let out = render_paths_grouped(&s, None);
        assert!(out.contains("length 2:"));
        assert!(out.contains("  a -> b -> c"));
    }

    #[test]
    fn grouped_same_length_paths_one_header() {
        let s = sp(
            "lib",
            "leaf",
            vec![vec!["lib", "a", "leaf"], vec!["lib", "b", "leaf"]],
        );
        let out = render_paths_grouped(&s, None);
        let headers: Vec<&str> = out.lines().filter(|l| l.starts_with("length")).collect();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], "length 2:");
    }

    // ---- render_paths_dot ---------------------------------------------------

    #[test]
    fn dot_empty_paths_returns_empty() {
        let e = edges(&[("a", "b")]);
        let s = sp("a", "b", vec![]);
        assert_eq!(render_paths_dot(&e, &s, None), "");
    }

    #[test]
    fn dot_path_edge_colored_red() {
        let e = edges(&[("lib", "a"), ("lib", "b"), ("a", "leaf"), ("b", "leaf")]);
        let s = sp("lib", "leaf", vec![vec!["lib", "a", "leaf"]]);
        let out = render_paths_dot(&e, &s, None);
        assert!(out.contains("\"lib\" -> \"a\" [color=red, style=bold, penwidth=2.0];"));
        assert!(out.contains("\"a\" -> \"leaf\" [color=red, style=bold, penwidth=2.0];"));
        assert!(out.contains("\"lib\" -> \"b\";"));
        assert!(out.contains("\"b\" -> \"leaf\";"));
    }

    #[test]
    fn dot_starts_and_ends_correctly() {
        let e = edges(&[("a", "b")]);
        let s = sp("a", "b", vec![vec!["a", "b"]]);
        let out = render_paths_dot(&e, &s, None);
        assert!(out.starts_with("digraph dependencies {"));
        assert!(out.ends_with("}\n"));
    }
}
