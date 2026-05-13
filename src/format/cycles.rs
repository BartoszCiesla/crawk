use super::deps_cmd::AnnotatedEdges;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// A strongly connected component with >=2 modules (a dependency cycle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cycle {
    /// Module paths in this SCC, sorted alphabetically.
    pub modules: BTreeSet<String>,
    /// Edges within this SCC, with optional API annotations preserved.
    pub edges: AnnotatedEdges,
}

// ---------------------------------------------------------------------------
// Tarjan's SCC — iterative
// ---------------------------------------------------------------------------

struct TarjanState<'a> {
    index_counter: usize,
    stack: Vec<&'a str>,
    on_stack: BTreeSet<&'a str>,
    index: BTreeMap<&'a str, usize>,
    lowlink: BTreeMap<&'a str, usize>,
    sccs: Vec<BTreeSet<&'a str>>,
}

struct Frame<'a> {
    node: &'a str,
    neighbors: Vec<&'a str>,
    neighbor_idx: usize,
}

fn tarjan_scc<'a>(adj: &BTreeMap<&'a str, Vec<&'a str>>) -> Vec<BTreeSet<&'a str>> {
    let mut state = TarjanState {
        index_counter: 0,
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        index: BTreeMap::new(),
        lowlink: BTreeMap::new(),
        sccs: Vec::new(),
    };

    for &node in adj.keys() {
        if !state.index.contains_key(node) {
            strongconnect(&mut state, node, adj);
        }
    }

    state.sccs
}

fn strongconnect<'a>(
    state: &mut TarjanState<'a>,
    start: &'a str,
    adj: &BTreeMap<&'a str, Vec<&'a str>>,
) {
    let mut call_stack: Vec<Frame<'a>> = Vec::new();

    // Initialize start node
    state.index.insert(start, state.index_counter);
    state.lowlink.insert(start, state.index_counter);
    state.index_counter += 1;
    state.stack.push(start);
    state.on_stack.insert(start);

    let neighbors = adj.get(start).map_or_else(Vec::new, Clone::clone);
    call_stack.push(Frame {
        node: start,
        neighbors,
        neighbor_idx: 0,
    });

    while let Some(frame) = call_stack.last_mut() {
        if frame.neighbor_idx < frame.neighbors.len() {
            let w = frame.neighbors[frame.neighbor_idx];
            frame.neighbor_idx += 1;

            if !state.index.contains_key(w) {
                // Recurse: initialize w and push new frame
                state.index.insert(w, state.index_counter);
                state.lowlink.insert(w, state.index_counter);
                state.index_counter += 1;
                state.stack.push(w);
                state.on_stack.insert(w);

                let w_neighbors = adj.get(w).map_or_else(Vec::new, Clone::clone);
                call_stack.push(Frame {
                    node: w,
                    neighbors: w_neighbors,
                    neighbor_idx: 0,
                });
            } else if state.on_stack.contains(w) {
                let v = frame.node;
                let w_index = state.index[w];
                let v_low = state.lowlink[v];
                if w_index < v_low {
                    state.lowlink.insert(v, w_index);
                }
            }
        } else {
            // Done with all neighbors — pop frame.
            // Safety: we just confirmed `last_mut()` returned `Some`, so `pop` is safe.
            let Some(finished) = call_stack.pop() else {
                break;
            };
            let v = finished.node;

            if let Some(parent_frame) = call_stack.last() {
                let parent = parent_frame.node;
                let v_low = state.lowlink[v];
                let p_low = state.lowlink[parent];
                if v_low < p_low {
                    state.lowlink.insert(parent, v_low);
                }
            }

            // If v is a root node, pop the SCC
            if state.lowlink[v] == state.index[v] {
                let mut scc = BTreeSet::new();
                while let Some(w) = state.stack.pop() {
                    state.on_stack.remove(w);
                    scc.insert(w);
                    if w == v {
                        break;
                    }
                }
                state.sccs.push(scc);
            }
        }
    }
}

/// Detect all dependency cycles in the module graph.
///
/// Uses Tarjan's strongly connected components algorithm to find groups
/// of mutually-dependent modules. Only SCCs with 2+ modules are returned.
/// Returns cycles sorted by their first module name (alphabetically).
pub(crate) fn detect_cycles(edges: &AnnotatedEdges) -> Vec<Cycle> {
    // Build adjacency list from edges
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (source, target) in edges.keys() {
        adj.entry(source.as_str())
            .or_default()
            .push(target.as_str());
        // Ensure target appears as a node even if it has no outgoing edges
        adj.entry(target.as_str()).or_default();
    }

    let sccs = tarjan_scc(&adj);

    let mut cycles: Vec<Cycle> = sccs
        .into_iter()
        .filter(|scc| scc.len() >= 2)
        .map(|scc| {
            let modules: BTreeSet<String> = scc.iter().map(|s| (*s).to_owned()).collect();
            // Extract edges where both endpoints are in this SCC
            let cycle_edges: AnnotatedEdges = edges
                .iter()
                .filter(|((s, t), _)| modules.contains(s.as_str()) && modules.contains(t.as_str()))
                .map(|((s, t), apis)| ((s.clone(), t.clone()), apis.clone()))
                .collect();
            Cycle {
                modules,
                edges: cycle_edges,
            }
        })
        .collect();

    // Sort by first module in each cycle
    cycles.sort_by(|a, b| {
        let a_first = a.modules.iter().next();
        let b_first = b.modules.iter().next();
        a_first.cmp(&b_first)
    });

    cycles
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

/// Format an API annotation suffix for a single edge.
fn format_api_suffix(apis: &BTreeSet<String>) -> String {
    if apis.is_empty() {
        return String::new();
    }
    let items: Vec<&str> = apis.iter().map(String::as_str).collect();
    format!(" [{}]", items.join(", "))
}

/// Render cycles as plain text.
///
/// ```text
/// cycle 1 (3 modules)
///   alpha -> beta
///   beta -> gamma
///   gamma -> alpha
/// ```
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

/// Render cycles as DOT — only cycle nodes and edges.
pub(crate) fn render_cycles_dot(cycles: &[Cycle]) -> String {
    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=box, style=rounded, fontname=\"monospace\", fontsize=10];\n");
    out.push_str("    edge [color=\"#444444\"];\n");

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

    // ---- detect_cycles -------------------------------------------------------

    #[test]
    fn detect_cycles_empty_graph() {
        let edges = make_edges(&[]);
        assert!(detect_cycles(&edges).is_empty());
    }

    #[test]
    fn detect_cycles_dag() {
        let edges = make_edges(&[("a", "b"), ("b", "c"), ("a", "c")]);
        assert!(detect_cycles(&edges).is_empty());
    }

    #[test]
    fn detect_cycles_simple_two_node() {
        let edges = make_edges(&[("a", "b"), ("b", "a")]);
        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 1);
        assert_eq!(
            cycles[0].modules,
            BTreeSet::from(["a".to_owned(), "b".to_owned()])
        );
    }

    #[test]
    fn detect_cycles_three_node() {
        let edges = make_edges(&[("a", "b"), ("b", "c"), ("c", "a")]);
        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 1);
        assert_eq!(
            cycles[0].modules,
            BTreeSet::from(["a".to_owned(), "b".to_owned(), "c".to_owned()])
        );
    }

    #[test]
    fn detect_cycles_two_independent() {
        let edges = make_edges(&[("a", "b"), ("b", "a"), ("c", "d"), ("d", "c")]);
        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 2);
        assert_eq!(
            cycles[0].modules,
            BTreeSet::from(["a".to_owned(), "b".to_owned()])
        );
        assert_eq!(
            cycles[1].modules,
            BTreeSet::from(["c".to_owned(), "d".to_owned()])
        );
    }

    #[test]
    fn detect_cycles_preserves_apis() {
        let edges = make_edges_with_apis(&[("a", "b", &["FooType"]), ("b", "a", &["BarType"])]);
        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 1);
        let ab_apis = &cycles[0].edges[&("a".to_owned(), "b".to_owned())];
        assert!(ab_apis.contains("FooType"));
    }

    #[test]
    fn detect_cycles_excludes_non_cycle_nodes() {
        let edges = make_edges(&[
            ("alpha", "beta"),
            ("beta", "gamma"),
            ("delta", "alpha"),
            ("gamma", "alpha"),
        ]);
        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 1);
        assert!(!cycles[0].modules.contains("delta"));
        assert_eq!(cycles[0].modules.len(), 3);
    }

    #[test]
    fn detect_cycles_sorted_by_first_module() {
        let edges = make_edges(&[("x", "y"), ("y", "x"), ("a", "b"), ("b", "a")]);
        let cycles = detect_cycles(&edges);
        assert_eq!(cycles.len(), 2);
        assert!(cycles[0].modules.contains("a"));
        assert!(cycles[1].modules.contains("x"));
    }

    // ---- render_plain --------------------------------------------------------

    #[test]
    fn render_plain_single_cycle() {
        let cycles = vec![Cycle {
            modules: BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            edges: make_edges(&[("a", "b"), ("b", "a")]),
        }];
        assert_eq!(
            render_cycles_plain(&cycles),
            "cycle 1 (2 modules)\n  a -> b\n  b -> a\n"
        );
    }

    #[test]
    fn render_plain_multiple_cycles() {
        let cycles = vec![
            Cycle {
                modules: BTreeSet::from(["a".to_owned(), "b".to_owned()]),
                edges: make_edges(&[("a", "b"), ("b", "a")]),
            },
            Cycle {
                modules: BTreeSet::from(["c".to_owned(), "d".to_owned()]),
                edges: make_edges(&[("c", "d"), ("d", "c")]),
            },
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
        let cycles = vec![Cycle {
            modules: BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            edges: make_edges_with_apis(&[("a", "b", &["FooType"]), ("b", "a", &["BarType"])]),
        }];
        let out = render_cycles_plain(&cycles);
        assert!(out.contains("a -> b [FooType]"));
        assert!(out.contains("b -> a [BarType]"));
    }

    // ---- render_grouped ------------------------------------------------------

    #[test]
    fn render_grouped_header_lists_modules() {
        let cycles = vec![Cycle {
            modules: BTreeSet::from(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]),
            edges: make_edges(&[("alpha", "beta"), ("beta", "gamma"), ("gamma", "alpha")]),
        }];
        let out = render_cycles_grouped(&cycles);
        assert!(out.contains("cycle 1 (3 modules): alpha, beta, gamma"));
    }

    // ---- render_dot ----------------------------------------------------------

    #[test]
    fn render_dot_detect_subgraph_clusters() {
        let cycles = vec![Cycle {
            modules: BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            edges: make_edges(&[("a", "b"), ("b", "a")]),
        }];
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
        let cycles = vec![Cycle {
            modules: BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            edges: make_edges(&[("a", "b"), ("b", "a")]),
        }];
        let out = render_cycles_dot_highlight(&cycles, &all);
        assert!(out.contains("\"a\" -> \"b\" [color=\"#e67700\", style=bold, penwidth=2.0]"));
        assert!(out.contains("\"c\" -> \"a\""));
    }

    #[test]
    fn render_dot_highlight_non_cycle_normal_color() {
        let all = make_edges(&[("a", "b"), ("b", "a"), ("c", "a")]);
        let cycles = vec![Cycle {
            modules: BTreeSet::from(["a".to_owned(), "b".to_owned()]),
            edges: make_edges(&[("a", "b"), ("b", "a")]),
        }];
        let out = render_cycles_dot_highlight(&cycles, &all);
        let ca_line = out
            .lines()
            .find(|l| l.contains("\"c\" -> \"a\""))
            .unwrap_or("");
        assert!(!ca_line.contains("#cc0000"));
    }
}
