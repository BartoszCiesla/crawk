use crawk::TypeReference;
use std::collections::BTreeSet;

pub(crate) mod flat;
pub(crate) mod grouped;
pub(crate) mod list;

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
