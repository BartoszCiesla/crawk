use std::collections::BTreeSet;
use std::fmt::Write;

use crawk::{AnnotatedEdges, Cycle};

use super::format_api_suffix;

/// Render cycles as plain text.
///
/// ```text
/// cycle 1 (3 modules)
///   alpha -> beta
///   beta -> gamma
///   gamma -> alpha
/// ```
#[must_use]
pub(crate) fn render_cycles_plain(cycles: &[Cycle]) -> String {
    if cycles.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, cycle) in cycles.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "cycle {} ({} modules)", i + 1, cycle.modules.len());
        for ((source, target), apis) in &cycle.edges {
            let _ = writeln!(out, "  {source} -> {target}{}", format_api_suffix(apis));
        }
    }
    out
}

/// Render cycles as grouped text with module names in header.
///
/// ```text
/// cycle 1 (3 modules): alpha, beta, gamma
///   alpha -> beta
///   beta -> gamma
///   gamma -> alpha
/// ```
#[must_use]
pub(crate) fn render_cycles_grouped(cycles: &[Cycle]) -> String {
    if cycles.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, cycle) in cycles.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let names: Vec<&str> = cycle.modules.iter().map(String::as_str).collect();
        let _ = writeln!(
            out,
            "cycle {} ({} modules): {}",
            i + 1,
            cycle.modules.len(),
            names.join(", ")
        );
        for ((source, target), apis) in &cycle.edges {
            let _ = writeln!(out, "  {source} -> {target}{}", format_api_suffix(apis));
        }
    }
    out
}

fn write_cycle_subgraphs(out: &mut String, cycles: &[Cycle]) {
    for (i, cycle) in cycles.iter().enumerate() {
        let _ = write!(out, "\n    subgraph cluster_cycle_{} {{\n", i + 1);
        let _ = writeln!(
            out,
            "        label=\"cycle {} ({} modules)\";",
            i + 1,
            cycle.modules.len()
        );
        out.push_str("        style=dashed;\n");
        out.push_str("        color=\"#e67700\";\n");
        out.push_str("        fontcolor=\"#e67700\";\n");
        for module in &cycle.modules {
            let _ = writeln!(out, "        \"{module}\";");
        }
        out.push_str("    }\n");
    }
}

/// Render cycles as DOT — only cycle nodes and edges.
#[must_use]
pub(crate) fn render_cycles_dot(cycles: &[Cycle]) -> String {
    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=rounded, fontname=\"monospace\", fontsize=10];\n");
    out.push_str("    edge [color=\"#444444\"];\n");

    write_cycle_subgraphs(&mut out, cycles);

    // Edges
    let has_edges = cycles.iter().any(|c| !c.edges.is_empty());
    if has_edges {
        out.push('\n');
        for cycle in cycles {
            for ((source, target), apis) in &cycle.edges {
                let _ = write!(
                    out,
                    "    \"{source}\" -> \"{target}\" [color=\"#e67700\", style=bold, penwidth=2.0"
                );
                if !apis.is_empty() {
                    let items: Vec<&str> = apis.iter().map(String::as_str).collect();
                    let _ = write!(
                        out,
                        ", label=\"{}\", fontcolor=\"#c96a00\"",
                        items.join(", ")
                    );
                }
                out.push_str("];\n");
            }
        }
    }

    out.push_str("}\n");
    out
}

/// Render full graph with cycle edges highlighted in red.
#[must_use]
pub(crate) fn render_cycles_dot_highlight(cycles: &[Cycle], all_edges: &AnnotatedEdges) -> String {
    // Collect all cycle edges for quick lookup
    let mut cycle_edge_set: BTreeSet<(&str, &str)> = BTreeSet::new();
    for cycle in cycles {
        for (source, target) in cycle.edges.keys() {
            cycle_edge_set.insert((source.as_str(), target.as_str()));
        }
    }

    // Collect cycle node set
    let mut cycle_nodes: BTreeSet<&str> = BTreeSet::new();
    for cycle in cycles {
        for module in &cycle.modules {
            cycle_nodes.insert(module.as_str());
        }
    }

    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=rounded, fontname=\"monospace\", fontsize=10];\n");
    out.push_str("    edge [color=\"#444444\"];\n");

    // Cycle clusters
    write_cycle_subgraphs(&mut out, cycles);

    // Non-cycle nodes
    let mut all_nodes: BTreeSet<&str> = BTreeSet::new();
    for (source, target) in all_edges.keys() {
        all_nodes.insert(source.as_str());
        all_nodes.insert(target.as_str());
    }
    let non_cycle: Vec<&str> = all_nodes
        .iter()
        .filter(|n| !cycle_nodes.contains(**n))
        .copied()
        .collect();
    if !non_cycle.is_empty() {
        out.push('\n');
        for node in &non_cycle {
            let _ = writeln!(out, "    \"{node}\";");
        }
    }

    // All edges
    out.push('\n');
    for ((source, target), apis) in all_edges {
        let is_cycle = cycle_edge_set.contains(&(source.as_str(), target.as_str()));
        let _ = write!(out, "    \"{source}\" -> \"{target}\"");
        if is_cycle {
            out.push_str(" [color=\"#e67700\", style=bold, penwidth=2.0");
            if !apis.is_empty() {
                let items: Vec<&str> = apis.iter().map(String::as_str).collect();
                let _ = write!(
                    out,
                    ", label=\"{}\", fontcolor=\"#c96a00\"",
                    items.join(", ")
                );
            }
            out.push(']');
        } else if !apis.is_empty() {
            let items: Vec<&str> = apis.iter().map(String::as_str).collect();
            let _ = write!(
                out,
                " [label=\"{}\", fontcolor=\"#c96a00\"]",
                items.join(", ")
            );
        }
        out.push_str(";\n");
    }

    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edges(pairs: &[(&str, &str)]) -> AnnotatedEdges {
        pairs
            .iter()
            .map(|(s, t)| ((s.to_string(), t.to_string()), BTreeSet::new()))
            .collect()
    }

    fn make_edges_with_apis(pairs: &[(&str, &str, &[&str])]) -> AnnotatedEdges {
        pairs
            .iter()
            .map(|(s, t, apis)| {
                let api_set: BTreeSet<String> = apis.iter().map(|a| (*a).to_owned()).collect();
                ((s.to_string(), t.to_string()), api_set)
            })
            .collect()
    }

    // ---- render_plain --------------------------------------------------------

    #[test]
    fn render_plain_single_cycle() {
        let cycles = vec![Cycle::new(
            BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            make_edges(&[("a", "b"), ("b", "a")]),
        )];
        assert_eq!(
            render_cycles_plain(&cycles),
            "cycle 1 (2 modules)\n  a -> b\n  b -> a\n"
        );
    }

    #[test]
    fn render_plain_multiple_cycles() {
        let cycles = vec![
            Cycle::new(
                BTreeSet::from(["a".to_owned(), "b".to_owned()]),
                make_edges(&[("a", "b"), ("b", "a")]),
            ),
            Cycle::new(
                BTreeSet::from(["c".to_owned(), "d".to_owned()]),
                make_edges(&[("c", "d"), ("d", "c")]),
            ),
        ];
        let out = render_cycles_plain(&cycles);
        assert!(out.contains("\n\ncycle 2"));
    }

    #[test]
    fn render_plain_no_cycles() {
        assert_eq!(render_cycles_plain(&[]), "");
    }

    #[test]
    fn render_plain_with_apis() {
        let cycles = vec![Cycle::new(
            BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            make_edges_with_apis(&[("a", "b", &["FooType"]), ("b", "a", &["BarType"])]),
        )];
        let out = render_cycles_plain(&cycles);
        assert!(out.contains("a -> b [FooType]"));
        assert!(out.contains("b -> a [BarType]"));
    }

    // ---- render_grouped ------------------------------------------------------

    #[test]
    fn render_grouped_header_lists_modules() {
        let cycles = vec![Cycle::new(
            BTreeSet::from(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]),
            make_edges(&[("alpha", "beta"), ("beta", "gamma"), ("gamma", "alpha")]),
        )];
        let out = render_cycles_grouped(&cycles);
        assert!(out.contains("cycle 1 (3 modules): alpha, beta, gamma"));
    }

    // ---- render_dot ----------------------------------------------------------

    #[test]
    fn render_dot_detect_subgraph_clusters() {
        let cycles = vec![Cycle::new(
            BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            make_edges(&[("a", "b"), ("b", "a")]),
        )];
        let out = render_cycles_dot(&cycles);
        assert!(out.contains("subgraph cluster_cycle_1"));
        assert!(out.contains("color=\"#e67700\""));
        assert!(out.contains("\"a\" -> \"b\" [color=\"#e67700\", style=bold, penwidth=2.0]"));
    }

    #[test]
    fn render_dot_detect_empty() {
        let out = render_cycles_dot(&[]);
        assert!(out.starts_with("digraph dependencies {"));
        assert!(out.ends_with("}\n"));
        assert!(!out.contains("subgraph"));
    }

    // ---- render_dot_highlight ------------------------------------------------

    #[test]
    fn render_dot_highlight_all_edges_present() {
        let all = make_edges(&[("a", "b"), ("b", "a"), ("c", "a")]);
        let cycles = vec![Cycle::new(
            BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            make_edges(&[("a", "b"), ("b", "a")]),
        )];
        let out = render_cycles_dot_highlight(&cycles, &all);
        assert!(out.contains("\"a\" -> \"b\" [color=\"#e67700\", style=bold, penwidth=2.0]"));
        assert!(out.contains("\"c\" -> \"a\""));
    }

    #[test]
    fn render_dot_highlight_non_cycle_normal_color() {
        let all = make_edges(&[("a", "b"), ("b", "a"), ("c", "a")]);
        let cycles = vec![Cycle::new(
            BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            make_edges(&[("a", "b"), ("b", "a")]),
        )];
        let out = render_cycles_dot_highlight(&cycles, &all);
        let ca_line = out
            .lines()
            .find(|l| l.contains("\"c\" -> \"a\""))
            .unwrap_or("");
        assert!(!ca_line.contains("#cc0000"));
    }
}
