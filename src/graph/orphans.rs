use std::collections::BTreeSet;

use super::edges::AnnotatedEdges;

/// Find orphan modules — modules with no incoming edges.
///
/// An orphan is a module present in `all_modules` that is never referenced as
/// a target by any edge. This includes target roots (lib, main) — they are
/// entry points that naturally have no dependents, but are still reported for
/// transparency.
///
/// `all_modules` is expected to be already depth-truncated by the caller so
/// that orphan detection operates at the same granularity as the rendered
/// dependency graph.
///
/// Returns orphan module paths in sorted order.
#[must_use]
pub(crate) fn find_orphans(edges: &AnnotatedEdges, all_modules: &BTreeSet<String>) -> Vec<String> {
    let mut has_incoming: BTreeSet<&str> = BTreeSet::new();
    for (_source, target) in edges.keys() {
        has_incoming.insert(target.as_str());
    }

    all_modules
        .iter()
        .map(String::as_str)
        .filter(|m| !has_incoming.contains(m))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn make_edges(pairs: &[(&str, &str)]) -> AnnotatedEdges {
        pairs
            .iter()
            .map(|(s, t)| ((s.to_string(), t.to_string()), BTreeSet::new()))
            .collect()
    }

    fn modules(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn empty_graph_no_orphans() {
        let result = find_orphans(&BTreeMap::new(), &BTreeSet::new());
        assert!(result.is_empty());
    }

    #[test]
    fn all_connected_no_orphans() {
        let edges = make_edges(&[("a", "b"), ("b", "a")]);
        let result = find_orphans(&edges, &modules(&["a", "b"]));
        assert!(result.is_empty());
    }

    #[test]
    fn source_only_is_orphan() {
        let edges = make_edges(&[("a", "b"), ("c", "b")]);
        let result = find_orphans(&edges, &modules(&["a", "b", "c"]));
        assert_eq!(result, vec!["a", "c"]);
    }

    #[test]
    fn root_is_orphan_too() {
        let edges = make_edges(&[("lib", "a"), ("standalone", "a")]);
        let result = find_orphans(&edges, &modules(&["a", "lib", "standalone"]));
        assert_eq!(result, vec!["lib", "standalone"]);
    }

    #[test]
    fn isolated_module_is_orphan() {
        let edges = make_edges(&[("a", "b")]);
        let result = find_orphans(&edges, &modules(&["a", "b", "isolated"]));
        assert_eq!(result, vec!["a", "isolated"]);
    }

    #[test]
    fn orphans_sorted_alphabetically() {
        let edges = make_edges(&[("zebra", "target"), ("alpha", "target")]);
        let result = find_orphans(&edges, &modules(&["alpha", "target", "zebra"]));
        assert_eq!(result, vec!["alpha", "zebra"]);
    }

    #[test]
    fn cycles_fixture_scenario() {
        let edges = make_edges(&[
            ("alpha", "beta"),
            ("beta", "gamma"),
            ("gamma", "alpha"),
            ("delta", "alpha"),
            ("standalone", "delta"),
        ]);
        let result = find_orphans(
            &edges,
            &modules(&["alpha", "beta", "delta", "gamma", "lib", "standalone"]),
        );
        assert_eq!(result, vec!["lib", "standalone"]);
    }
}
