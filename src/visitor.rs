use crate::expansion::{
    expand_path_to_string, expand_use_tree, extract_public_items, is_internal_path,
    is_internal_use, is_test_module,
};
use crate::formatter::{
    expand_use_tree_to_paths, strip_crate_prefix, truncate_path, use_tree_to_string,
};
use crate::resolver::resolve_module_path_to_file;
use proc_macro2::Span;
use std::collections::HashSet;
use std::path::PathBuf;
use syn::UseTree;
use syn::visit::{self, Visit};
use tracing::debug;

#[allow(clippy::struct_excessive_bools)]
pub struct UseVisitor<'a> {
    pub use_statements: &'a mut HashSet<String>,
    pub module_path: Vec<String>,
    pub src_dir: PathBuf,
    pub include_tests: bool,
    pub in_test_module: bool,
    pub expand: bool,
    pub depth: Option<usize>,
}

impl<'a> Visit<'a> for UseVisitor<'a> {
    fn visit_item_use(&mut self, node: &'a syn::ItemUse) {
        // Skip use statements inside test modules unless include_tests is true
        if !self.in_test_module || self.include_tests {
            // Check if this is an internal crate use (self::, super::, or crate::)
            if is_internal_use(&node.tree) {
                // Expand self and super to full module paths
                let mut expanded_tree = expand_use_tree(&node.tree, &self.module_path);

                // Expand globs to explicit items
                expanded_tree = self.expand_globs(&expanded_tree);

                if self.expand {
                    // Expand groups into individual paths
                    let expanded_paths = expand_use_tree_to_paths(&expanded_tree);
                    for path in expanded_paths {
                        let truncated = truncate_path(&path, self.depth);
                        let without_crate = strip_crate_prefix(&truncated);
                        self.use_statements.insert(without_crate);
                    }
                } else {
                    let use_string = use_tree_to_string(&expanded_tree);
                    let truncated = truncate_path(&use_string, self.depth);
                    let without_crate = strip_crate_prefix(&truncated);
                    self.use_statements.insert(without_crate);
                }
            }
        }

        visit::visit_item_use(self, node);
    }

    fn visit_item_mod(&mut self, node: &'a syn::ItemMod) {
        let was_in_test = self.in_test_module;

        // Check if this module is a test module
        if !self.include_tests && is_test_module(node) {
            self.in_test_module = true;
        }

        // Continue visiting
        visit::visit_item_mod(self, node);

        // Restore previous state
        self.in_test_module = was_in_test;
    }

    fn visit_expr_path(&mut self, node: &'a syn::ExprPath) {
        // Skip paths inside test modules unless include_tests is true
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }

        visit::visit_expr_path(self, node);
    }

    /// Visit type paths - captures type annotations like `let x: crate::Foo`,
    /// function parameters, return types, struct field types, generic bounds, etc.
    fn visit_type_path(&mut self, node: &'a syn::TypePath) {
        if !self.in_test_module || self.include_tests {
            // TypePath has an optional qself (qualified self) and a path
            // e.g., <T as crate::Trait>::Item or just crate::Foo
            self.process_path(&node.path);

            // Also check the qself if present (e.g., <crate::Foo as Trait>::Item)
            if let Some(qself) = &node.qself {
                // The type in qself will be visited recursively
                visit::visit_type(&mut *self, &qself.ty);
            }
        }

        visit::visit_type_path(self, node);
    }

    /// Visit pattern structs - captures struct patterns in match arms
    /// e.g., `match x { crate::Foo { field } => ... }`
    fn visit_pat_struct(&mut self, node: &'a syn::PatStruct) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }

        visit::visit_pat_struct(self, node);
    }

    /// Visit pattern tuple structs - captures tuple struct patterns
    /// e.g., `match x { crate::Foo(a, b) => ... }`
    fn visit_pat_tuple_struct(&mut self, node: &'a syn::PatTupleStruct) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }

        visit::visit_pat_tuple_struct(self, node);
    }

    /// Visit struct expressions - captures struct literal construction
    /// e.g., `crate::Foo { field: value }`
    fn visit_expr_struct(&mut self, node: &'a syn::ExprStruct) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }

        visit::visit_expr_struct(self, node);
    }

    /// Visit trait bounds - captures trait bounds in generics
    /// e.g., `fn foo<T: crate::MyTrait>()`
    fn visit_trait_bound(&mut self, node: &'a syn::TraitBound) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }

        visit::visit_trait_bound(self, node);
    }

    /// Visit impl items - captures impl blocks
    /// e.g., `impl crate::Trait for Foo` or `impl crate::Foo`
    fn visit_item_impl(&mut self, node: &'a syn::ItemImpl) {
        if !self.in_test_module || self.include_tests {
            // Check the trait being implemented (if any)
            if let Some((_, trait_path, _)) = &node.trait_ {
                self.process_path(trait_path);
            }
        }

        visit::visit_item_impl(self, node);
    }

    /// Visit macro invocations - captures macro paths
    /// e.g., `crate::my_macro!()`
    fn visit_macro(&mut self, node: &'a syn::Macro) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }

        visit::visit_macro(self, node);
    }
}

impl UseVisitor<'_> {
    /// Process a path and add it to use_statements if it's an internal crate path
    fn process_path(&mut self, path: &syn::Path) {
        if is_internal_path(path) {
            let expanded = expand_path_to_string(path, &self.module_path);
            let truncated = truncate_path(&expanded, self.depth);
            let without_crate = strip_crate_prefix(&truncated);
            self.use_statements.insert(without_crate);
        }
    }

    fn expand_globs(&self, tree: &UseTree) -> UseTree {
        self.expand_globs_with_path(tree, &[])
    }

    fn expand_globs_with_path(&self, tree: &UseTree, accumulated_path: &[String]) -> UseTree {
        match tree {
            UseTree::Path(path) => {
                let mut new_path = accumulated_path.to_vec();
                new_path.push(path.ident.to_string());

                let expanded_subtree = self.expand_globs_with_path(&path.tree, &new_path);
                UseTree::Path(syn::UsePath {
                    ident: path.ident.clone(),
                    colon2_token: path.colon2_token,
                    tree: Box::new(expanded_subtree),
                })
            }
            UseTree::Glob(_glob) => {
                // accumulated_path contains the module path (e.g., ["crate", "foo", "bar"])
                // Need to resolve this to a file and get public items
                self.resolve_glob_items(accumulated_path)
                    .unwrap_or_else(|| tree.clone())
            }
            UseTree::Group(group) => {
                let expanded_items: syn::punctuated::Punctuated<UseTree, syn::Token![,]> = group
                    .items
                    .iter()
                    .map(|item| self.expand_globs_with_path(item, accumulated_path))
                    .collect();
                UseTree::Group(syn::UseGroup {
                    brace_token: group.brace_token,
                    items: expanded_items,
                })
            }
            _ => tree.clone(),
        }
    }

    fn resolve_glob_items(&self, module_path: &[String]) -> Option<UseTree> {
        debug!("Attempting to resolve glob for path: {module_path:?}");

        // Resolve the module path to a file
        let module_file = if let Some(f) = resolve_module_path_to_file(&self.src_dir, module_path) {
            debug!("Resolved glob path to file: {}", f.display());
            f
        } else {
            debug!("Failed to resolve module path to file");
            return None;
        };

        // Parse the file and extract public items
        let public_items = if let Some(items) = extract_public_items(&module_file) {
            debug!("Found {} public items in module", items.len());
            items
        } else {
            debug!("Failed to extract public items from file");
            return None;
        };

        if public_items.is_empty() {
            debug!("No public items found in module");
            return None;
        }

        // Create a group with all public items
        let items: syn::punctuated::Punctuated<UseTree, syn::Token![,]> = public_items
            .into_iter()
            .map(|name| {
                UseTree::Name(syn::UseName {
                    ident: syn::Ident::new(&name, Span::call_site()),
                })
            })
            .collect();

        Some(UseTree::Group(syn::UseGroup {
            brace_token: syn::token::Brace::default(),
            items,
        }))
    }
}
