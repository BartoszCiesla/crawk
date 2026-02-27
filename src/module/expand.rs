use crate::constants::{
    ATTR_CFG, MODULE_NAME_TEST, MODULE_NAME_TESTS, PATH_QUALIFIER_CRATE, PATH_QUALIFIER_SELF,
    PATH_QUALIFIER_SUPER,
};
use proc_macro2::Span;
use std::fs;
use std::path::Path;
use syn::{Item, UseTree};

/// Expand self and super references in a UseTree to absolute paths
#[must_use]
pub fn expand_use_tree(tree: &UseTree, module_path: &[String]) -> UseTree {
    match tree {
        UseTree::Path(path) => {
            let ident_str = path.ident.to_string();

            if ident_str == PATH_QUALIFIER_SELF {
                // Replace self with crate::module::path
                if module_path.is_empty() {
                    // self at crate root becomes crate
                    UseTree::Path(syn::UsePath {
                        ident: syn::Ident::new(PATH_QUALIFIER_CRATE, path.ident.span()),
                        colon2_token: path.colon2_token,
                        tree: Box::new(expand_use_tree(&path.tree, module_path)),
                    })
                } else {
                    // Build crate::module::path::rest
                    build_expanded_path(module_path, &path.tree)
                }
            } else if ident_str == PATH_QUALIFIER_SUPER {
                // Replace super with parent module path
                if module_path.is_empty() {
                    // super at crate root is invalid, but keep as-is
                    UseTree::Path(syn::UsePath {
                        ident: path.ident.clone(),
                        colon2_token: path.colon2_token,
                        tree: Box::new(expand_use_tree(&path.tree, module_path)),
                    })
                } else {
                    // Go up one level
                    let parent_path = &module_path[..module_path.len() - 1];
                    build_expanded_path(parent_path, &path.tree)
                }
            } else if ident_str == PATH_QUALIFIER_CRATE {
                // crate stays as crate
                UseTree::Path(syn::UsePath {
                    ident: path.ident.clone(),
                    colon2_token: path.colon2_token,
                    tree: Box::new(expand_use_tree(&path.tree, module_path)),
                })
            } else {
                // Regular path component
                UseTree::Path(syn::UsePath {
                    ident: path.ident.clone(),
                    colon2_token: path.colon2_token,
                    tree: Box::new(expand_use_tree(&path.tree, module_path)),
                })
            }
        }
        UseTree::Name(name) => UseTree::Name(name.clone()),
        UseTree::Rename(rename) => UseTree::Rename(rename.clone()),
        UseTree::Glob(glob) => UseTree::Glob(glob.clone()),
        UseTree::Group(group) => {
            let expanded_items: syn::punctuated::Punctuated<UseTree, syn::Token![,]> = group
                .items
                .iter()
                .map(|item| expand_use_tree(item, module_path))
                .collect();
            UseTree::Group(syn::UseGroup {
                brace_token: group.brace_token,
                items: expanded_items,
            })
        }
    }
}

/// Build an expanded path from module path components and remaining tree
fn build_expanded_path(module_path: &[String], rest: &UseTree) -> UseTree {
    // Build the path from right to left: rest is the innermost part
    let mut result = expand_use_tree(rest, &[]);

    // Wrap with module path components from right to left
    for module_name in module_path.iter().rev() {
        result = UseTree::Path(syn::UsePath {
            ident: syn::Ident::new(module_name, Span::call_site()),
            colon2_token: syn::Token![::](Span::call_site()),
            tree: Box::new(result),
        });
    }

    // Wrap with crate at the top
    UseTree::Path(syn::UsePath {
        ident: syn::Ident::new(PATH_QUALIFIER_CRATE, Span::call_site()),
        colon2_token: syn::Token![::](Span::call_site()),
        tree: Box::new(result),
    })
}

/// Check if a UseTree represents an internal crate use (self, super, or crate)
#[must_use]
pub fn is_internal_use(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => {
            let ident = path.ident.to_string();
            ident == PATH_QUALIFIER_SELF
                || ident == PATH_QUALIFIER_SUPER
                || ident == PATH_QUALIFIER_CRATE
        }
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            ident == PATH_QUALIFIER_SELF
                || ident == PATH_QUALIFIER_SUPER
                || ident == PATH_QUALIFIER_CRATE
        }
        UseTree::Rename(rename) => {
            let ident = rename.ident.to_string();
            ident == PATH_QUALIFIER_SELF
                || ident == PATH_QUALIFIER_SUPER
                || ident == PATH_QUALIFIER_CRATE
        }
        UseTree::Glob(_) => false,
        UseTree::Group(group) => {
            // Check if any item in the group is internal
            group.items.iter().any(is_internal_use)
        }
    }
}

/// Check if a module is a test module
#[must_use]
pub fn is_test_module(item_mod: &syn::ItemMod) -> bool {
    let module_name = item_mod.ident.to_string();

    // Check if module name is "test" or "tests"
    if module_name == MODULE_NAME_TEST || module_name == MODULE_NAME_TESTS {
        return true;
    }

    // Check for #[cfg(test)] attribute
    for attr in &item_mod.attrs {
        if attr.path().is_ident(ATTR_CFG)
            && let Ok(meta_list) = attr.meta.require_list()
        {
            let tokens = meta_list.tokens.to_string();
            if tokens == MODULE_NAME_TEST {
                return true;
            }
        }
    }

    false
}

/// Extract public items from a module file
#[must_use]
pub fn extract_public_items(file_path: &Path) -> Option<Vec<String>> {
    let content = fs::read_to_string(file_path).ok()?;
    let file = syn::parse_file(&content).ok()?;

    let mut public_items = Vec::new();

    for item in &file.items {
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
                // Handle pub use re-exports
                if matches!(use_item.vis, syn::Visibility::Public(_)) {
                    extract_use_names(&use_item.tree, &mut public_items);
                }
            }
            _ => {}
        }
    }

    Some(public_items)
}

/// Check if a syn::Path represents an internal crate reference (crate::, self::, or super::)
///
/// A bare `self` (single segment) is a method receiver, not a module path,
/// so it is excluded. Internal module paths always have at least two segments
/// (e.g., `self::foo`, `crate::bar`, `super::baz`).
#[must_use]
pub fn is_internal_path(path: &syn::Path) -> bool {
    path.segments.len() > 1
        && path.segments.first().is_some_and(|first_segment| {
            let ident = first_segment.ident.to_string();
            ident == PATH_QUALIFIER_SELF
                || ident == PATH_QUALIFIER_SUPER
                || ident == PATH_QUALIFIER_CRATE
        })
}

/// Expand a syn::Path (from expressions) to a full path string, resolving self/super
#[must_use]
pub fn expand_path_to_string(path: &syn::Path, module_path: &[String]) -> String {
    let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();

    if segments.is_empty() {
        return String::new();
    }

    let first = &segments[0];
    let rest = &segments[1..];

    if first == PATH_QUALIFIER_CRATE {
        // crate::foo::bar -> crate::foo::bar
        segments.join("::")
    } else if first == PATH_QUALIFIER_SELF {
        // self::foo -> crate::module_path::foo
        let mut result = vec![PATH_QUALIFIER_CRATE.to_string()];
        result.extend(module_path.iter().cloned());
        result.extend(rest.iter().cloned());
        result.join("::")
    } else if first == PATH_QUALIFIER_SUPER {
        // super::foo -> crate::parent_path::foo
        let mut result = vec![PATH_QUALIFIER_CRATE.to_string()];
        if !module_path.is_empty() {
            result.extend(module_path[..module_path.len() - 1].iter().cloned());
        }
        result.extend(rest.iter().cloned());
        result.join("::")
    } else {
        // External path, return as-is
        segments.join("::")
    }
}

/// Extract names from a use tree (for pub use re-exports)
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
            // Can't expand nested globs
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_is_internal_use_crate() {
        let tree: UseTree = parse_quote!(crate::foo);
        assert!(is_internal_use(&tree));
    }

    #[test]
    fn test_is_internal_use_self() {
        let tree: UseTree = parse_quote!(self::foo);
        assert!(is_internal_use(&tree));
    }

    #[test]
    fn test_is_internal_use_super() {
        let tree: UseTree = parse_quote!(super::foo);
        assert!(is_internal_use(&tree));
    }

    #[test]
    fn test_is_internal_use_external() {
        let tree: UseTree = parse_quote!(std::collections::HashMap);
        assert!(!is_internal_use(&tree));
    }

    #[test]
    fn test_is_internal_use_group_with_internal() {
        let tree: UseTree = parse_quote!({crate::foo, std::bar});
        assert!(is_internal_use(&tree));
    }

    #[test]
    fn test_is_internal_use_glob() {
        let tree: UseTree = parse_quote!(std::*);
        assert!(!is_internal_use(&tree));
    }

    #[test]
    fn test_expand_use_tree_self() {
        let tree: UseTree = parse_quote!(self::foo::Bar);
        let module_path = vec!["utils".to_string()];
        let expanded = expand_use_tree(&tree, &module_path);
        let expanded_str = crate::module::format::use_tree_to_string(&expanded);
        assert_eq!(expanded_str, "crate::utils::foo::Bar");
    }

    #[test]
    fn test_expand_use_tree_super() {
        let tree: UseTree = parse_quote!(super::sibling::Item);
        let module_path = vec!["parent".to_string(), "child".to_string()];
        let expanded = expand_use_tree(&tree, &module_path);
        let expanded_str = crate::module::format::use_tree_to_string(&expanded);
        assert_eq!(expanded_str, "crate::parent::sibling::Item");
    }

    #[test]
    fn test_expand_use_tree_crate() {
        let tree: UseTree = parse_quote!(crate::foo::Bar);
        let module_path = vec!["utils".to_string()];
        let expanded = expand_use_tree(&tree, &module_path);
        let expanded_str = crate::module::format::use_tree_to_string(&expanded);
        assert_eq!(expanded_str, "crate::foo::Bar");
    }

    #[test]
    fn test_expand_use_tree_group() {
        let tree: UseTree = parse_quote!(self::{foo, bar});
        let module_path = vec!["utils".to_string()];
        let expanded = expand_use_tree(&tree, &module_path);
        let expanded_str = crate::module::format::use_tree_to_string(&expanded);
        assert_eq!(expanded_str, "crate::utils::{foo, bar}");
    }

    #[test]
    fn test_is_test_module_cfg_test() {
        let item: syn::ItemMod = parse_quote! {
            #[cfg(test)]
            mod tests {
            }
        };
        assert!(is_test_module(&item));
    }

    #[test]
    fn test_is_test_module_name_test() {
        let item: syn::ItemMod = parse_quote! {
            mod test {
            }
        };
        assert!(is_test_module(&item));
    }

    #[test]
    fn test_is_test_module_name_tests() {
        let item: syn::ItemMod = parse_quote! {
            mod tests {
            }
        };
        assert!(is_test_module(&item));
    }

    #[test]
    fn test_is_test_module_regular() {
        let item: syn::ItemMod = parse_quote! {
            mod regular_module {
            }
        };
        assert!(!is_test_module(&item));
    }

    #[test]
    fn test_extract_public_items_from_test_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
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

        let items = extract_public_items(temp_file.path()).unwrap();

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
    }

    #[test]
    fn test_extract_public_items_nonexistent_file() {
        let result = extract_public_items(Path::new("/nonexistent/file.rs"));
        assert!(result.is_none());
    }

    #[test]
    fn test_is_internal_path_crate() {
        let path: syn::Path = parse_quote!(crate::foo::bar);
        assert!(is_internal_path(&path));
    }

    #[test]
    fn test_is_internal_path_self() {
        let path: syn::Path = parse_quote!(self::foo);
        assert!(is_internal_path(&path));
    }

    #[test]
    fn test_is_internal_path_super() {
        let path: syn::Path = parse_quote!(super::foo);
        assert!(is_internal_path(&path));
    }

    #[test]
    fn test_is_internal_path_external() {
        let path: syn::Path = parse_quote!(std::collections::HashMap);
        assert!(!is_internal_path(&path));
    }

    #[test]
    fn test_expand_path_to_string_crate() {
        let path: syn::Path = parse_quote!(crate::foo::bar);
        let result = expand_path_to_string(&path, &["utils".to_string()]);
        assert_eq!(result, "crate::foo::bar");
    }

    #[test]
    fn test_expand_path_to_string_self() {
        let path: syn::Path = parse_quote!(self::helper::Thing);
        let result = expand_path_to_string(&path, &["utils".to_string()]);
        assert_eq!(result, "crate::utils::helper::Thing");
    }

    #[test]
    fn test_expand_path_to_string_super() {
        let path: syn::Path = parse_quote!(super::sibling::Item);
        let result = expand_path_to_string(&path, &["parent".to_string(), "child".to_string()]);
        assert_eq!(result, "crate::parent::sibling::Item");
    }

    #[test]
    fn test_expand_path_to_string_external() {
        let path: syn::Path = parse_quote!(std::collections::HashMap);
        let result = expand_path_to_string(&path, &["utils".to_string()]);
        assert_eq!(result, "std::collections::HashMap");
    }
}
