use syn::visit::Visit;
use syn::{ItemMod, ItemUse, UseTree};

use crate::constants::{PATH_QUALIFIER_CRATE, PATH_QUALIFIER_SELF, PATH_QUALIFIER_SUPER};
use crate::reference::{GroupItem, PathPrefix, TypeReference};
use crate::utils::has_cfg_test;

/// Visitor for extracting type references from module AST.
pub(super) struct ModuleVisitor {
    /// Module name for filtering and identification (e.g., "foo::bar").
    /// Used to match against nested module declarations. Empty string means analyze all modules.
    module_name: String,

    /// Module path as segments for resolving relative paths (e.g., `["foo", "bar"]`).
    /// Used to convert `self::` and `super::` references to absolute `crate::` paths.
    module_path: Vec<String>,

    /// Collected type references found in this module.
    /// All relative paths are resolved to absolute paths before being added.
    pub(super) references: Vec<TypeReference>,

    /// Track if we're currently visiting inside a test module.
    /// Updated as we traverse nested modules to filter test-only references.
    in_test_module: bool,
}

impl ModuleVisitor {
    pub(super) fn new(module_name: impl Into<String>) -> Self {
        let module_name = module_name.into();
        let module_path: Vec<String> = if module_name.is_empty() {
            vec![]
        } else {
            module_name.split("::").map(String::from).collect()
        };

        Self {
            module_name,
            module_path,
            references: Vec::new(),
            in_test_module: false,
        }
    }

    /// Checks if a syn::Path is an internal crate reference.
    /// Returns true if the path starts with crate::, self::, or super::.
    fn is_internal_path(path: &syn::Path) -> bool {
        path.segments.first().is_some_and(|first_segment| {
            let ident = first_segment.ident.to_string();
            matches!(
                ident.as_str(),
                PATH_QUALIFIER_CRATE | PATH_QUALIFIER_SELF | PATH_QUALIFIER_SUPER
            )
        })
    }

    /// Processes a syn::Path and converts it to a TypeReference if it's internal.
    fn process_path(&mut self, path: &syn::Path) {
        if !Self::is_internal_path(path) {
            return;
        }

        let mut segments = Vec::new();
        let mut path_prefix = PathPrefix::None;

        for (i, segment) in path.segments.iter().enumerate() {
            let ident = segment.ident.to_string();

            // Handle special prefixes at the start
            if i == 0 {
                match ident.as_str() {
                    PATH_QUALIFIER_CRATE => {
                        path_prefix = PathPrefix::Crate;
                        continue;
                    }
                    PATH_QUALIFIER_SELF => {
                        path_prefix = PathPrefix::SelfModule;
                        continue;
                    }
                    PATH_QUALIFIER_SUPER => {
                        path_prefix = PathPrefix::Super(1);
                        continue;
                    }
                    _ => {}
                }
            } else if ident == PATH_QUALIFIER_SUPER {
                // Handle chained super::
                let levels = match path_prefix {
                    PathPrefix::Super(n) => n + 1,
                    _ => 1,
                };
                path_prefix = PathPrefix::Super(levels);
                continue;
            }

            segments.push(ident);
        }

        if !segments.is_empty() {
            let reference = TypeReference::new(segments)
                .with_prefix(path_prefix)
                .resolve(&self.module_path);
            self.references.push(reference);
        }
    }

    fn process_use_tree(&mut self, tree: &UseTree, prefix: Vec<String>, path_prefix: PathPrefix) {
        match tree {
            UseTree::Path(p) => {
                let ident = p.ident.to_string();

                // Check for special prefixes at the start
                let (new_prefix, new_path_prefix) = if prefix.is_empty() {
                    match ident.as_str() {
                        PATH_QUALIFIER_CRATE => (Vec::new(), PathPrefix::Crate),
                        PATH_QUALIFIER_SELF => (Vec::new(), PathPrefix::SelfModule),
                        PATH_QUALIFIER_SUPER => {
                            let levels = match path_prefix {
                                PathPrefix::Super(n) => n + 1,
                                _ => 1,
                            };
                            (Vec::new(), PathPrefix::Super(levels))
                        }
                        _ => {
                            let mut new_prefix = prefix;
                            new_prefix.push(ident);
                            (new_prefix, path_prefix)
                        }
                    }
                } else if ident == PATH_QUALIFIER_SUPER {
                    // Handle chained super:: in the middle of path
                    let levels = match path_prefix {
                        PathPrefix::Super(n) => n + 1,
                        _ => 1,
                    };
                    (prefix, PathPrefix::Super(levels))
                } else {
                    let mut new_prefix = prefix;
                    new_prefix.push(ident);
                    (new_prefix, path_prefix)
                };

                self.process_use_tree(&p.tree, new_prefix, new_path_prefix);
            }

            UseTree::Name(n) => {
                let mut segments = prefix;
                segments.push(n.ident.to_string());

                let reference = TypeReference::new(segments)
                    .with_prefix(path_prefix)
                    .resolve(&self.module_path);
                self.references.push(reference);
            }

            UseTree::Rename(r) => {
                let mut segments = prefix;
                segments.push(r.ident.to_string());

                let reference = TypeReference::new(segments)
                    .with_prefix(path_prefix)
                    .with_alias(r.rename.to_string())
                    .resolve(&self.module_path);
                self.references.push(reference);
            }

            UseTree::Glob(_) => {
                let reference = TypeReference::new(prefix)
                    .with_prefix(path_prefix)
                    .with_glob()
                    .resolve(&self.module_path);
                self.references.push(reference);
            }

            UseTree::Group(g) => {
                let group_items = self.convert_group(&g.items);

                let reference = TypeReference::new(prefix)
                    .with_prefix(path_prefix)
                    .with_group(group_items)
                    .resolve(&self.module_path);
                self.references.push(reference);
            }
        }
    }

    fn convert_group(
        &self,
        items: &syn::punctuated::Punctuated<UseTree, syn::token::Comma>,
    ) -> Vec<GroupItem> {
        items
            .iter()
            .map(|item| self.convert_use_tree(item))
            .collect()
    }

    fn convert_use_tree(&self, tree: &UseTree) -> GroupItem {
        match tree {
            UseTree::Name(n) => {
                let ident = n.ident.to_string();
                if ident == PATH_QUALIFIER_SELF {
                    GroupItem::SelfItem { alias: None }
                } else {
                    GroupItem::Simple(ident)
                }
            }

            UseTree::Rename(r) => {
                let ident = r.ident.to_string();
                let alias = r.rename.to_string();
                if ident == PATH_QUALIFIER_SELF {
                    GroupItem::SelfItem { alias: Some(alias) }
                } else {
                    GroupItem::Aliased { name: ident, alias }
                }
            }

            UseTree::Glob(_) => GroupItem::Glob,

            UseTree::Path(p) => {
                let mut prefix = vec![p.ident.to_string()];
                let mut current = &*p.tree;

                // Flatten nested paths
                while let UseTree::Path(inner) = current {
                    prefix.push(inner.ident.to_string());
                    current = &*inner.tree;
                }

                match current {
                    UseTree::Group(g) => GroupItem::Nested {
                        prefix,
                        items: self.convert_group(&g.items),
                    },
                    UseTree::Name(n) => {
                        prefix.push(n.ident.to_string());
                        GroupItem::Nested {
                            prefix,
                            items: Vec::new(),
                        }
                    }
                    UseTree::Rename(r) => {
                        prefix.push(r.ident.to_string());
                        GroupItem::Nested {
                            prefix: prefix[..prefix.len() - 1].to_vec(),
                            items: vec![GroupItem::Aliased {
                                name: r.ident.to_string(),
                                alias: r.rename.to_string(),
                            }],
                        }
                    }
                    UseTree::Glob(_) => GroupItem::Nested {
                        prefix,
                        items: vec![GroupItem::Glob],
                    },
                    UseTree::Path(_) => GroupItem::Nested {
                        prefix,
                        items: Vec::new(),
                    },
                }
            }

            UseTree::Group(g) => GroupItem::Nested {
                prefix: Vec::new(),
                items: self.convert_group(&g.items),
            },
        }
    }
}

impl<'ast> Visit<'ast> for ModuleVisitor {
    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        let was_in_test = self.in_test_module;

        // Check if this module is a test module (has #[cfg(test)] attribute)
        if has_cfg_test(&i.attrs) {
            self.in_test_module = true;
        }

        // Check if this module matches the target module or is on the path to it
        let module_ident = i.ident.to_string();
        let should_visit = if self.module_name.is_empty() {
            // No filter, visit all modules
            true
        } else if self.module_name == module_ident {
            // Exact match
            true
        } else if self.module_name.contains("::") {
            // For nested modules like "foo::bar::baz", check if this module
            // is on the path (e.g., ident is "foo" and module_name starts with "foo::")
            // or if this module is the final segment
            self.module_name.split("::").any(|seg| seg == module_ident)
                || self.module_name.ends_with(&format!("::{module_ident}"))
        } else {
            // Single-segment module name, check exact match
            self.module_name == module_ident
        };

        if should_visit && let Some((_, items)) = &i.content {
            for item in items {
                self.visit_item(item);
            }
        }

        // Restore previous test module state
        self.in_test_module = was_in_test;
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        if !self.in_test_module {
            self.process_use_tree(&node.tree, Vec::new(), PathPrefix::None);
        }
    }

    /// Visit expression paths - captures paths in expressions like `crate::foo::bar()`
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if !self.in_test_module {
            self.process_path(&node.path);
        }
        syn::visit::visit_expr_path(self, node);
    }

    /// Visit type paths - captures type annotations like `let x: crate::Foo`
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if !self.in_test_module {
            self.process_path(&node.path);

            // Also check the qself if present (e.g., <crate::Foo as Trait>::Item)
            if let Some(qself) = &node.qself {
                syn::visit::visit_type(self, &qself.ty);
            }
        }
        syn::visit::visit_type_path(self, node);
    }

    /// Visit pattern structs - captures struct patterns in match arms
    fn visit_pat_struct(&mut self, node: &'ast syn::PatStruct) {
        if !self.in_test_module {
            self.process_path(&node.path);
        }
        syn::visit::visit_pat_struct(self, node);
    }

    /// Visit pattern tuple structs - captures tuple struct patterns
    fn visit_pat_tuple_struct(&mut self, node: &'ast syn::PatTupleStruct) {
        if !self.in_test_module {
            self.process_path(&node.path);
        }
        syn::visit::visit_pat_tuple_struct(self, node);
    }

    /// Visit struct expressions - captures struct literal construction
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if !self.in_test_module {
            self.process_path(&node.path);
        }
        syn::visit::visit_expr_struct(self, node);
    }

    /// Visit trait bounds - captures trait bounds in generics
    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        if !self.in_test_module {
            self.process_path(&node.path);
        }
        syn::visit::visit_trait_bound(self, node);
    }

    /// Visit impl items - captures impl blocks
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if !self.in_test_module {
            // Check the trait being implemented (if any)
            if let Some((_, trait_path, _)) = &node.trait_ {
                self.process_path(trait_path);
            }
        }
        syn::visit::visit_item_impl(self, node);
    }

    /// Visit macro invocations - captures macro paths
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if !self.in_test_module {
            self.process_path(&node.path);
        }
        syn::visit::visit_macro(self, node);
    }
}
