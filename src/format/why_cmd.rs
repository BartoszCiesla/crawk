use std::collections::{BTreeMap, BTreeSet, HashSet};

use crawk::TypeReference;

/// Render a flat sorted list of references explaining a dependency.
///
/// All references across source submodules are merged, deduplicated, and
/// sorted by their path string. Each line is a full `crate::…` path.
#[must_use]
pub(crate) fn render_plain(refs: &BTreeMap<String, HashSet<TypeReference>>) -> String {
    let mut all: BTreeSet<String> = BTreeSet::new();
    for type_refs in refs.values() {
        for r in type_refs {
            all.insert(r.to_path_string());
        }
    }
    let mut out = String::new();
    for path in &all {
        out.push_str(path);
        out.push('\n');
    }
    out
}

/// Render references grouped by source submodule.
///
/// Each source module appears as a header, followed by its references
/// indented with two spaces. Groups are separated by blank lines and
/// sorted alphabetically by module name.
#[must_use]
pub(crate) fn render_grouped(refs: &BTreeMap<String, HashSet<TypeReference>>) -> String {
    let mut out = String::new();
    let mut first = true;
    for (module, type_refs) in refs {
        if !first {
            out.push('\n');
        }
        first = false;
        out.push_str(module);
        out.push('\n');

        let mut paths: Vec<String> = type_refs
            .iter()
            .map(TypeReference::to_path_string)
            .collect();
        paths.sort();
        for path in &paths {
            out.push_str("  ");
            out.push_str(path);
            out.push('\n');
        }
    }
    out
}
