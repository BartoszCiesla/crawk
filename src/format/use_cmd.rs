use std::collections::BTreeSet;

use crawk::{AnalysisResult, TypeReference};

fn truncate_and_dedup<'a>(
    refs: impl IntoIterator<Item = &'a TypeReference>,
    depth: Option<usize>,
) -> Vec<String> {
    refs.into_iter()
        .map(|r| depth.map_or_else(|| r.to_string(), |d| r.truncate_to_depth(d).to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Renders dependencies as a flat, alphabetically sorted list.
///
/// Each dependency occupies one line. Duplicates produced by depth truncation are
/// removed. Returns an empty string when there are no dependencies.
pub(crate) fn render_flat(result: &AnalysisResult, depth: Option<usize>) -> String {
    let mut output = truncate_and_dedup(result.dependencies().values().flatten(), depth).join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

/// Renders dependencies grouped by source module, sorted alphabetically by module name.
///
/// Each module name appears as a header line, followed by its dependencies indented
/// with ` - `. Within each group, duplicates produced by depth truncation are removed
/// and entries are sorted alphabetically.
pub(crate) fn render_grouped(result: &AnalysisResult, depth: Option<usize>) -> String {
    let dependencies = result.dependencies();
    let mut modules = dependencies.keys().cloned().collect::<Vec<_>>();
    modules.sort();

    let mut out = String::new();
    for module in modules {
        let display_name = if module.is_empty() {
            result.module_path()
        } else {
            module.as_str()
        };
        out.push_str(display_name);
        out.push('\n');
        for reference in truncate_and_dedup(&dependencies[&module], depth) {
            out.push_str(" - ");
            out.push_str(&reference);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_identical_refs() {
        let r = TypeReference::new(["crate", "foo", "Bar"]);
        let result = truncate_and_dedup([&r, &r], None);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn sorts_alphabetically() {
        let r1 = TypeReference::new(["z", "last"]);
        let r2 = TypeReference::new(["a", "first"]);
        let result = truncate_and_dedup([&r1, &r2], None);
        assert_eq!(result[0], "a::first");
        assert_eq!(result[1], "z::last");
    }

    #[test]
    fn applies_depth_truncation() {
        let r = TypeReference::new(["a", "b", "c"]);
        let result = truncate_and_dedup([&r], Some(2));
        assert_eq!(result, vec!["a::b"]);
    }

    #[test]
    fn depth_truncation_deduplicates_after_truncate() {
        let r1 = TypeReference::new(["a", "b", "c"]);
        let r2 = TypeReference::new(["a", "b", "d"]);
        let result = truncate_and_dedup([&r1, &r2], Some(2));
        assert_eq!(result, vec!["a::b"]);
    }

    #[test]
    fn none_depth_preserves_full_path() {
        let r = TypeReference::new(["a", "b", "c"]);
        let result = truncate_and_dedup([&r], None);
        assert_eq!(result, vec!["a::b::c"]);
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = truncate_and_dedup(std::iter::empty::<&TypeReference>(), None);
        assert!(result.is_empty());
    }
}
