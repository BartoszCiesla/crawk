use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::edges::AnnotatedEdges;

/// All shortest dependency paths between two modules.
///
/// Produced by [`DependencyGraph::shortest_paths`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShortestPaths {
    /// Source module (start of every path).
    pub source: String,
    /// Target module (end of every path).
    pub target: String,
    /// All shortest paths from `source` to `target`, sorted lexicographically
    /// by `path.join(" -> ")`. Each inner `Vec` contains module names in order
    /// from source (index 0) to target (last index).
    pub paths: Vec<Vec<String>>,
}

impl ShortestPaths {
    #[must_use]
    pub const fn new(source: String, target: String, paths: Vec<Vec<String>>) -> Self {
        Self {
            source,
            target,
            paths,
        }
    }

    /// Returns `true` when no paths were found.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Length of the shortest paths (number of hops), or `None` if empty.
    #[must_use]
    pub fn length(&self) -> Option<usize> {
        self.paths.first().map(|p| p.len().saturating_sub(1))
    }
}

/// Compute all shortest paths from `source` to `target` using BFS.
///
/// `nodes` must contain both `source` and `target` (caller's responsibility).
/// Returns a [`ShortestPaths`] with empty `paths` when no path exists.
/// When `source == target`, returns a single path containing only `source`.
#[must_use]
pub(crate) fn compute_shortest_paths(
    edges: &AnnotatedEdges,
    nodes: &BTreeSet<String>,
    source: &str,
    target: &str,
) -> ShortestPaths {
    if source == target {
        return ShortestPaths::new(
            source.to_owned(),
            target.to_owned(),
            vec![vec![source.to_owned()]],
        );
    }

    // Build adjacency list from edges (only for nodes that exist in the node set).
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (src, tgt) in edges.keys() {
        if nodes.contains(src.as_str()) && nodes.contains(tgt.as_str()) {
            adj.entry(src.as_str()).or_default().push(tgt.as_str());
        }
    }

    // Level-by-level BFS: each iteration processes one full frontier level,
    // so all alternate predecessors at the same depth are captured naturally.
    let mut dist: HashMap<&str, usize> = HashMap::new();
    let mut preds: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut level: Vec<&str> = vec![source];

    dist.insert(source, 0);

    'bfs: while !level.is_empty() {
        let mut next: Vec<&str> = Vec::new();
        let mut found = false;
        for &v in &level {
            let v_dist = dist[v];
            for &w in adj.get(v).map_or(&[] as &[&str], Vec::as_slice) {
                match dist.get(w) {
                    None => {
                        dist.insert(w, v_dist + 1);
                        preds.entry(w).or_default().push(v);
                        next.push(w);
                        if w == target {
                            found = true;
                        }
                    }
                    Some(&d) if d == v_dist + 1 => {
                        preds.entry(w).or_default().push(v);
                    }
                    _ => {}
                }
            }
        }
        if found {
            break 'bfs;
        }
        level = next;
    }

    if !dist.contains_key(target) {
        return ShortestPaths::new(source.to_owned(), target.to_owned(), vec![]);
    }

    // Backtrack from target to source via predecessors.
    let mut paths: Vec<Vec<String>> = Vec::new();
    backtrack(target, source, &preds, &mut Vec::new(), &mut paths);

    // Sort lexicographically by "a -> b -> c" string.
    paths.sort_by_key(|p| p.join(" -> "));
    ShortestPaths::new(source.to_owned(), target.to_owned(), paths)
}

fn backtrack<'a>(
    node: &'a str,
    source: &'a str,
    preds: &HashMap<&'a str, Vec<&'a str>>,
    current: &mut Vec<&'a str>,
    result: &mut Vec<Vec<String>>,
) {
    current.push(node);
    if node == source {
        let mut path: Vec<String> = current.iter().copied().map(str::to_owned).collect();
        path.reverse();
        result.push(path);
    } else if let Some(ps) = preds.get(node) {
        for &pred in ps {
            backtrack(pred, source, preds, current, result);
        }
    }
    current.pop();
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn edges(pairs: &[(&str, &str)]) -> AnnotatedEdges {
        pairs
            .iter()
            .map(|(s, t)| ((s.to_string(), t.to_string()), BTreeSet::new()))
            .collect()
    }

    fn nodes(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn direct_edge() {
        let e = edges(&[("a", "b")]);
        let n = nodes(&["a", "b"]);
        let sp = compute_shortest_paths(&e, &n, "a", "b");
        assert_eq!(sp.paths, vec![vec!["a".to_owned(), "b".to_owned()]]);
    }

    #[test]
    fn transitive_path() {
        let e = edges(&[("a", "b"), ("b", "c")]);
        let n = nodes(&["a", "b", "c"]);
        let sp = compute_shortest_paths(&e, &n, "a", "c");
        assert_eq!(
            sp.paths,
            vec![vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]]
        );
    }

    #[test]
    fn diamond_two_shortest_paths() {
        // lib -> a -> leaf, lib -> b -> leaf
        let e = edges(&[("lib", "a"), ("lib", "b"), ("a", "leaf"), ("b", "leaf")]);
        let n = nodes(&["lib", "a", "b", "leaf"]);
        let sp = compute_shortest_paths(&e, &n, "lib", "leaf");
        assert_eq!(sp.paths.len(), 2);
        assert_eq!(
            sp.paths[0],
            vec!["lib".to_owned(), "a".to_owned(), "leaf".to_owned()]
        );
        assert_eq!(
            sp.paths[1],
            vec!["lib".to_owned(), "b".to_owned(), "leaf".to_owned()]
        );
    }

    #[test]
    fn source_equals_target() {
        let e = edges(&[("a", "b")]);
        let n = nodes(&["a", "b"]);
        let sp = compute_shortest_paths(&e, &n, "a", "a");
        assert_eq!(sp.paths, vec![vec!["a".to_owned()]]);
    }

    #[test]
    fn no_path_returns_empty() {
        let e = edges(&[("a", "b")]);
        let n = nodes(&["a", "b"]);
        let sp = compute_shortest_paths(&e, &n, "b", "a");
        assert!(sp.is_empty());
    }

    #[test]
    fn cycle_no_infinite_loop() {
        let e = edges(&[("a", "b"), ("b", "a"), ("a", "c")]);
        let n = nodes(&["a", "b", "c"]);
        let sp = compute_shortest_paths(&e, &n, "a", "c");
        assert_eq!(sp.paths, vec![vec!["a".to_owned(), "c".to_owned()]]);
    }

    #[test]
    fn shortest_paths_is_empty() {
        let sp = ShortestPaths::new("a".to_owned(), "b".to_owned(), vec![]);
        assert!(sp.is_empty());
        assert!(sp.length().is_none());
    }

    #[test]
    fn shortest_paths_length() {
        let sp = ShortestPaths::new(
            "a".to_owned(),
            "c".to_owned(),
            vec![vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]],
        );
        assert!(!sp.is_empty());
        assert_eq!(sp.length(), Some(2));
    }

    #[test]
    fn shortest_paths_single_node() {
        let sp = ShortestPaths::new("a".to_owned(), "a".to_owned(), vec![vec!["a".to_owned()]]);
        assert!(!sp.is_empty());
        assert_eq!(sp.length(), Some(0));
    }

    #[test]
    fn paths_sorted_lexicographically() {
        // b path would sort before a path lexicographically by "lib -> b -> leaf"
        let e = edges(&[("lib", "a"), ("lib", "b"), ("a", "leaf"), ("b", "leaf")]);
        let n = nodes(&["lib", "a", "b", "leaf"]);
        let sp = compute_shortest_paths(&e, &n, "lib", "leaf");
        let joined: Vec<String> = sp.paths.iter().map(|p| p.join(" -> ")).collect();
        let mut sorted = joined.clone();
        sorted.sort();
        assert_eq!(joined, sorted);
    }

    #[test]
    fn empty_graph_no_path() {
        let e = BTreeMap::new();
        let n = nodes(&["a", "b"]);
        let sp = compute_shortest_paths(&e, &n, "a", "b");
        assert!(sp.is_empty());
    }
}
