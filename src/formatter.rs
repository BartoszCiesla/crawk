use crate::consts::PATH_QUALIFIER_CRATE;
use syn::UseTree;

/// Convert a UseTree to its string representation
#[must_use]
pub fn use_tree_to_string(tree: &UseTree) -> String {
    match tree {
        UseTree::Path(path) => {
            format!("{}::{}", path.ident, use_tree_to_string(&path.tree))
        }
        UseTree::Name(name) => name.ident.to_string(),
        UseTree::Rename(rename) => {
            format!("{} as {}", rename.ident, rename.rename)
        }
        UseTree::Glob(_) => "*".to_string(),
        UseTree::Group(group) => {
            let items: Vec<String> = group.items.iter().map(use_tree_to_string).collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

/// Expand a UseTree into individual paths (flattens groups)
#[must_use]
pub fn expand_use_tree_to_paths(tree: &UseTree) -> Vec<String> {
    match tree {
        UseTree::Path(path) => {
            let prefix = path.ident.to_string();
            let suffixes = expand_use_tree_to_paths(&path.tree);

            suffixes
                .into_iter()
                .map(|suffix| format!("{prefix}::{suffix}"))
                .collect()
        }
        UseTree::Name(name) => {
            vec![name.ident.to_string()]
        }
        UseTree::Rename(rename) => {
            vec![format!("{} as {}", rename.ident, rename.rename)]
        }
        UseTree::Glob(_) => {
            vec!["*".to_string()]
        }
        UseTree::Group(group) => {
            let mut all_paths = Vec::new();
            for item in &group.items {
                all_paths.extend(expand_use_tree_to_paths(item));
            }
            all_paths
        }
    }
}

/// Remove "crate::" prefix from a path
#[must_use]
pub fn strip_crate_prefix(path: &str) -> String {
    path.strip_prefix(format!("{PATH_QUALIFIER_CRATE}::").as_str())
        .unwrap_or(path)
        .to_string()
}

/// Truncate a path to a specified depth from crate root
#[must_use]
pub fn truncate_path(path: &str, depth: Option<usize>) -> String {
    let Some(depth) = depth else {
        return path.to_string();
    };

    // Split by :: to get components
    let parts: Vec<&str> = path.split("::").collect();

    // If the path starts with "crate", count from there
    if parts.first() == Some(&PATH_QUALIFIER_CRATE) {
        // depth 1 means crate::x, depth 2 means crate::x::y, etc.
        // So we need depth + 1 components (including "crate")
        let take_count = (depth + 1).min(parts.len());
        parts
            .iter()
            .take(take_count)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("::")
    } else {
        // For non-crate paths, just take the first 'depth' components
        let take_count = depth.min(parts.len());
        parts
            .iter()
            .take(take_count)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("::")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_crate_prefix() {
        assert_eq!(strip_crate_prefix("crate::foo::bar"), "foo::bar");
        assert_eq!(strip_crate_prefix("foo::bar"), "foo::bar");
        assert_eq!(strip_crate_prefix("crate::foo"), "foo");
    }

    #[test]
    fn test_truncate_path() {
        assert_eq!(truncate_path("crate::foo::bar::baz", Some(1)), "crate::foo");
        assert_eq!(
            truncate_path("crate::foo::bar::baz", Some(2)),
            "crate::foo::bar"
        );
        assert_eq!(
            truncate_path("crate::foo::bar::baz", Some(3)),
            "crate::foo::bar::baz"
        );
        assert_eq!(
            truncate_path("crate::foo::bar::baz", None),
            "crate::foo::bar::baz"
        );
    }

    #[test]
    fn test_truncate_path_overflow() {
        assert_eq!(truncate_path("crate::foo", Some(10)), "crate::foo");
    }
}
