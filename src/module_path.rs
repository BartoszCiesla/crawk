//! Vocabulary for crawk-internal module paths.
//!
//! A module path is segments joined by `::`, with the empty string denoting the
//! crate root and no `crate::` prefix — the format produced by [`discover`] and
//! consumed by [`analyzer`], [`resolve`], [`rules`] and [`graph`].
//!
//! The predicates here are deliberately the *only* implementations in the crate:
//! subtree membership and parent extraction were previously open-coded at four
//! and three sites respectively, with two mechanisms and a semantic split on the
//! empty-ancestor case.
//!
//! [`discover`]: crate::discover
//! [`analyzer`]: crate::analyzer
//! [`resolve`]: crate::resolve
//! [`rules`]: crate::rules
//! [`graph`]: crate::graph

/// Returns `true` if `module` lies in the subtree rooted at `ancestor`,
/// including `ancestor` itself.
///
/// Matching is on `::` segment boundaries, so `foo` covers `foo::bar` but never
/// `foobar`. An empty `ancestor` denotes the crate root, whose subtree is every
/// module in the crate.
///
/// - `("foo::bar", "foo")` → `true`
/// - `("foo", "foo")`      → `true`
/// - `("foobar", "foo")`   → `false`
/// - `("foo::bar", "")`    → `true`
/// - `("", "foo")`         → `false`
pub(crate) fn is_in_subtree(module: &str, ancestor: &str) -> bool {
    ancestor.is_empty()
        || module == ancestor
        || module
            .strip_prefix(ancestor)
            .is_some_and(|rest| rest.starts_with("::"))
}

/// Splits `module` into its parent path and its own name.
///
/// A top-level module has the crate root — the empty string — as its parent.
///
/// - `"foo::bar::baz"` → `("foo::bar", "baz")`
/// - `"foo::bar"`      → `("foo", "bar")`
/// - `"foo"`           → `("", "foo")`
/// - `""`              → `("", "")`
pub(crate) fn split_parent(module: &str) -> (&str, &str) {
    module.rsplit_once("::").unwrap_or(("", module))
}

/// The parent module of `module`, or `""` for a top-level module and for the
/// crate root itself.
///
/// The root answer is a convenience, not a claim: `pub(super)` at the crate root
/// is a rustc error, so no caller needs a meaningful value there.
pub(crate) fn parent_module(module: &str) -> &str {
    split_parent(module).0
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_in_subtree ---

    #[test]
    fn is_in_subtree_semantics() {
        // Empty ancestor = crate root: every module is in its subtree.
        assert!(is_in_subtree("", ""));
        assert!(is_in_subtree("foo", ""));
        assert!(is_in_subtree("foo::bar", ""));

        // Exact match and descendants are in the subtree.
        assert!(is_in_subtree("foo", "foo"));
        assert!(is_in_subtree("foo::bar", "foo"));
        assert!(is_in_subtree("foo::bar::baz", "foo"));

        // Prefix-only matches are NOT enough (no `::` boundary).
        assert!(!is_in_subtree("foobar", "foo"));
        assert!(!is_in_subtree("baz", "foo"));
        assert!(!is_in_subtree("", "foo"));
    }

    #[test]
    fn is_in_subtree_matches_on_multi_segment_ancestor() {
        assert!(is_in_subtree("foo::bar::baz", "foo::bar"));
        assert!(!is_in_subtree("foo::barbaz", "foo::bar"));
    }

    #[test]
    fn is_in_subtree_is_not_symmetric() {
        assert!(is_in_subtree("foo::bar", "foo"));
        assert!(!is_in_subtree("foo", "foo::bar"));
    }

    // --- split_parent / parent_module ---

    #[test]
    fn split_parent_separates_nested_path() {
        assert_eq!(split_parent("foo::bar"), ("foo", "bar"));
        assert_eq!(split_parent("foo::bar::baz"), ("foo::bar", "baz"));
    }

    #[test]
    fn split_parent_roots_top_level_module() {
        assert_eq!(split_parent("foo"), ("", "foo"));
    }

    #[test]
    fn split_parent_of_root_is_empty() {
        assert_eq!(split_parent(""), ("", ""));
    }

    #[test]
    fn parent_module_handles_root_and_nested() {
        assert_eq!(parent_module("foo::bar"), "foo");
        assert_eq!(parent_module("foo::bar::baz"), "foo::bar");
        assert_eq!(parent_module("foo"), "");
        assert_eq!(parent_module(""), "");
    }

    #[test]
    fn parent_is_an_ancestor_of_its_child() {
        let child = "foo::bar::baz";
        assert!(is_in_subtree(child, parent_module(child)));
    }
}
