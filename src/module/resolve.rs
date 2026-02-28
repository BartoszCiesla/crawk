//! Resolve glob imports by extracting the public API from a module's source file.
//!
//! Given a path to a `.rs` file, [`extract_public_items`] returns the names of
//! all publicly visible items (structs, enums, functions, constants, traits,
//! type aliases, modules, statics, and `pub use` re-exports).

use std::fs;
use std::path::Path;

use syn::{Item, UseTree};

/// Extract all public item names from a Rust source file.
///
/// Returns `None` if the file cannot be read or parsed.
/// Returns `Some(vec)` with the names of every `pub` item in the file.
/// For `pub use` re-exports, the *imported* name (or alias) is included.
///
/// When `inline_module` is non-empty, descends into the named inline module(s)
/// before extracting items. For example, `&["constants"]` extracts only from
/// `pub mod constants { ... }` within the file.
#[must_use]
pub fn extract_public_items(file_path: &Path, inline_module: &[&str]) -> Option<Vec<String>> {
    let content = fs::read_to_string(file_path).ok()?;
    let file = syn::parse_file(&content).ok()?;

    let items = if inline_module.is_empty() {
        &file.items
    } else {
        return extract_from_inline_module(&file.items, inline_module);
    };

    Some(collect_public_items(items))
}

/// Descend into nested inline modules and extract public items from the target.
fn extract_from_inline_module(items: &[Item], module_path: &[&str]) -> Option<Vec<String>> {
    if module_path.is_empty() {
        return Some(collect_public_items(items));
    }

    let target = module_path[0];
    for item in items {
        if let Item::Mod(item_mod) = item
            && item_mod.ident == target
            && let Some((_, nested_items)) = &item_mod.content
        {
            return extract_from_inline_module(nested_items, &module_path[1..]);
        }
    }

    // Inline module not found
    None
}

/// Collect names of all `pub` items from a list of syn items.
fn collect_public_items(items: &[Item]) -> Vec<String> {
    let mut public_items = Vec::new();

    for item in items {
        match item {
            Item::Fn(func) => {
                if matches!(func.vis, syn::Visibility::Public(_)) {
                    public_items.push(func.sig.ident.to_string());
                }
            }
            Item::Struct(struct_item) => {
                if matches!(struct_item.vis, syn::Visibility::Public(_)) {
                    public_items.push(struct_item.ident.to_string());
                }
            }
            Item::Enum(enum_item) => {
                if matches!(enum_item.vis, syn::Visibility::Public(_)) {
                    public_items.push(enum_item.ident.to_string());
                }
            }
            Item::Const(const_item) => {
                if matches!(const_item.vis, syn::Visibility::Public(_)) {
                    public_items.push(const_item.ident.to_string());
                }
            }
            Item::Static(static_item) => {
                if matches!(static_item.vis, syn::Visibility::Public(_)) {
                    public_items.push(static_item.ident.to_string());
                }
            }
            Item::Type(type_item) => {
                if matches!(type_item.vis, syn::Visibility::Public(_)) {
                    public_items.push(type_item.ident.to_string());
                }
            }
            Item::Mod(mod_item) => {
                if matches!(mod_item.vis, syn::Visibility::Public(_)) {
                    public_items.push(mod_item.ident.to_string());
                }
            }
            Item::Trait(trait_item) => {
                if matches!(trait_item.vis, syn::Visibility::Public(_)) {
                    public_items.push(trait_item.ident.to_string());
                }
            }
            Item::Use(use_item) => {
                if matches!(use_item.vis, syn::Visibility::Public(_)) {
                    extract_use_names(&use_item.tree, &mut public_items);
                }
            }
            _ => {}
        }
    }

    public_items
}

/// Recursively extract imported names from a `UseTree`.
///
/// For `pub use foo::Bar` the name `Bar` is collected.
/// For `pub use foo::Bar as Baz` the alias `Baz` is collected.
/// For `pub use foo::{A, B}` both `A` and `B` are collected.
/// Glob re-exports (`pub use foo::*`) are skipped because they cannot be
/// resolved without additional context.
fn extract_use_names(tree: &UseTree, items: &mut Vec<String>) {
    match tree {
        UseTree::Name(name) => {
            items.push(name.ident.to_string());
        }
        UseTree::Rename(rename) => {
            items.push(rename.rename.to_string());
        }
        UseTree::Path(path) => {
            extract_use_names(&path.tree, items);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                extract_use_names(item, items);
            }
        }
        UseTree::Glob(_) => {
            // Can't resolve nested globs without further context
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn extracts_all_public_item_kinds() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub struct PublicStruct;
struct PrivateStruct;
pub fn public_function() {{}}
fn private_function() {{}}
pub const PUBLIC_CONST: i32 = 42;
const PRIVATE_CONST: i32 = 42;
pub enum PublicEnum {{ A, B }}
enum PrivateEnum {{ X, Y }}
pub type PublicType = String;
type PrivateType = String;
pub mod public_module {{}}
mod private_module {{}}
pub trait PublicTrait {{}}
trait PrivateTrait {{}}
pub use std::collections::HashMap;
"
        )
        .unwrap();

        let items = extract_public_items(f.path(), &[]).unwrap();

        assert!(items.contains(&"PublicStruct".to_string()));
        assert!(items.contains(&"public_function".to_string()));
        assert!(items.contains(&"PUBLIC_CONST".to_string()));
        assert!(items.contains(&"PublicEnum".to_string()));
        assert!(items.contains(&"PublicType".to_string()));
        assert!(items.contains(&"public_module".to_string()));
        assert!(items.contains(&"PublicTrait".to_string()));
        assert!(items.contains(&"HashMap".to_string()));

        assert!(!items.contains(&"PrivateStruct".to_string()));
        assert!(!items.contains(&"private_function".to_string()));
        assert!(!items.contains(&"PRIVATE_CONST".to_string()));
        assert!(!items.contains(&"PrivateEnum".to_string()));
        assert!(!items.contains(&"PrivateType".to_string()));
        assert!(!items.contains(&"private_module".to_string()));
        assert!(!items.contains(&"PrivateTrait".to_string()));
    }

    #[test]
    fn extracts_pub_static() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub static PUB_STATIC: u32 = 1;
static PRIV_STATIC: u32 = 2;
"
        )
        .unwrap();

        let items = extract_public_items(f.path(), &[]).unwrap();
        assert!(items.contains(&"PUB_STATIC".to_string()));
        assert!(!items.contains(&"PRIV_STATIC".to_string()));
    }

    #[test]
    fn extracts_pub_use_rename() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub use std::collections::HashMap as Map;
"
        )
        .unwrap();

        let items = extract_public_items(f.path(), &[]).unwrap();
        assert!(items.contains(&"Map".to_string()));
        assert!(!items.contains(&"HashMap".to_string()));
    }

    #[test]
    fn extracts_pub_use_group() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub use std::collections::{{HashMap, HashSet}};
"
        )
        .unwrap();

        let items = extract_public_items(f.path(), &[]).unwrap();
        assert!(items.contains(&"HashMap".to_string()));
        assert!(items.contains(&"HashSet".to_string()));
    }

    #[test]
    fn returns_none_for_nonexistent_file() {
        let result = extract_public_items(Path::new("/nonexistent/file.rs"), &[]);
        assert!(result.is_none());
    }

    #[test]
    fn returns_empty_for_no_public_items() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
struct Private;
fn helper() {{}}
"
        )
        .unwrap();

        let items = extract_public_items(f.path(), &[]).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn extracts_items_from_inline_module() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub fn top_level() {{}}

pub mod inner {{
    pub fn inner_func() {{}}
    pub struct InnerStruct;
    fn private_in_inner() {{}}
}}
"
        )
        .unwrap();

        // Extract from root — should see top_level and inner (the module)
        let root_items = extract_public_items(f.path(), &[]).unwrap();
        assert!(root_items.contains(&"top_level".to_string()));
        assert!(root_items.contains(&"inner".to_string()));

        // Extract from inline module "inner"
        let inner_items = extract_public_items(f.path(), &["inner"]).unwrap();
        assert!(inner_items.contains(&"inner_func".to_string()));
        assert!(inner_items.contains(&"InnerStruct".to_string()));
        assert!(!inner_items.contains(&"private_in_inner".to_string()));
        assert!(!inner_items.contains(&"top_level".to_string()));
    }

    #[test]
    fn extracts_items_from_nested_inline_module() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub mod outer {{
    pub mod inner {{
        pub const DEEP: u32 = 42;
        pub fn deep_fn() {{}}
    }}
    pub fn outer_fn() {{}}
}}
"
        )
        .unwrap();

        let items = extract_public_items(f.path(), &["outer", "inner"]).unwrap();
        assert!(items.contains(&"DEEP".to_string()));
        assert!(items.contains(&"deep_fn".to_string()));
        assert!(!items.contains(&"outer_fn".to_string()));
    }

    #[test]
    fn returns_none_for_missing_inline_module() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub fn top_level() {{}}
"
        )
        .unwrap();

        let result = extract_public_items(f.path(), &["nonexistent"]);
        assert!(result.is_none());
    }
}
