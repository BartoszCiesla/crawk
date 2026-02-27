#![allow(dead_code)]
//! Module for parsing Rust source files and extracting type references.
//!
//! Provides [`CrateAnalyzer`] for collecting [`TypeReference`]s from multiple
//! source files within a crate.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{File, ItemMod, ItemUse, UseTree};
use thiserror::Error;

use super::path::{GroupItem, PathPrefix, PathSuffix, TypeReference};
use crate::constants::{
    ATTR_CFG, MODULE_NAME_TEST, PATH_QUALIFIER_CRATE, PATH_QUALIFIER_SELF, PATH_QUALIFIER_SUPER,
};

/// Errors that can occur during analysis.
#[derive(Debug, Error)]
pub enum AnalyzerError {
    /// Failed to read source file.
    #[error("Failed to read file '{path}': {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Failed to parse source file.
    #[error("Failed to parse file '{path}': {message}")]
    Parse { path: PathBuf, message: String },
}

/// Result type for analyzer operations.
pub type Result<T> = std::result::Result<T, AnalyzerError>;

/// Collected type references from a single source file.
#[derive(Debug, Clone, Default)]
pub struct FileReferences {
    /// Path to the source file.
    pub file_path: PathBuf,

    /// All type references found in this file.
    pub references: Vec<TypeReference>,
}

impl FileReferences {
    /// Creates a new `FileReferences` for the given file path.
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
        Self {
            file_path: file_path.into(),
            references: Vec::new(),
        }
    }

    /// Adds a type reference.
    pub fn push(&mut self, reference: TypeReference) {
        self.references.push(reference);
    }

    /// Returns the number of references.
    pub const fn len(&self) -> usize {
        self.references.len()
    }

    /// Returns true if no references were found.
    pub const fn is_empty(&self) -> bool {
        self.references.is_empty()
    }
}

/// Analyzer for collecting type references from a Rust crate.
#[derive(Debug, Clone)]
pub struct CrateAnalyzer {
    /// Name of the crate being analyzed.
    crate_name: String,

    /// Collected references per file.
    files: HashMap<String, FileReferences>,

    /// Order in which files were parsed (for deterministic iteration).
    file_order: Vec<String>,
}

impl CrateAnalyzer {
    /// Creates a new analyzer for the given crate.
    pub fn new(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            files: HashMap::new(),
            file_order: Vec::new(),
        }
    }

    /// Returns the crate name.
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    /// Parses a single source file and collects type references.
    pub fn parse_file(
        &mut self,
        module: impl Into<String>,
        path: &Path,
    ) -> Result<Vec<TypeReference>> {
        let content = std::fs::read_to_string(path).map_err(|e| AnalyzerError::FileRead {
            path: path.to_path_buf(),
            source: e,
        })?;

        let syntax: File = syn::parse_file(&content).map_err(|e| AnalyzerError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        let module = module.into();
        let mut visitor = ModuleVisitor::new(module.clone());
        visitor.visit_file(&syntax);

        let mut file_refs = FileReferences::new(path);
        let result = visitor.references.clone();
        file_refs.references = visitor.references;

        if !self.files.contains_key(&module) {
            self.file_order.push(module.clone());
        }
        self.files.insert(module, file_refs);

        Ok(result)
    }

    /// Returns all collected references by module, in parse order.
    pub fn all_references(&self) -> impl Iterator<Item = (&String, &FileReferences)> {
        self.file_order
            .iter()
            .filter_map(|module| self.files.get(module).map(|refs| (module, refs)))
    }

    /// Returns all collected crate internal references by module, in parse order.
    pub fn all_crate_references(&self) -> impl Iterator<Item = (&String, Vec<&TypeReference>)> {
        self.file_order.iter().filter_map(|module| {
            self.files.get(module).map(|refs| {
                let crate_refs: Vec<&TypeReference> = refs
                    .references
                    .iter()
                    .filter(|r| r.is_relative() || r.is_from_crate(&self.crate_name))
                    .collect();
                (module, crate_refs)
            })
        })
    }

    /// Returns an iterator over all references across all files.
    pub fn iter_references(&self) -> impl Iterator<Item = &TypeReference> {
        self.files.values().flat_map(|f| f.references.iter())
    }

    /// Returns an iterator over all crate internal references across all files.
    pub fn iter_crate_references(&self) -> impl Iterator<Item = &TypeReference> {
        self.files.values().flat_map(|f| {
            f.references
                .iter()
                .filter(|r| r.is_relative() || r.is_from_crate(&self.crate_name))
        })
    }

    /// Returns total number of references across all files.
    pub fn total_references(&self) -> usize {
        self.files.values().map(|f| f.references.len()).sum()
    }

    /// Returns number of parsed files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Clears all collected data.
    pub fn clear(&mut self) {
        self.files.clear();
        self.file_order.clear();
    }
}

/// Visitor for extracting type references from module AST.
struct ModuleVisitor {
    /// Module name for filtering and identification (e.g., "foo::bar").
    /// Used to match against nested module declarations. Empty string means analyze all modules.
    module_name: String,

    /// Module path as segments for resolving relative paths (e.g., `["foo", "bar"]`).
    /// Used to convert `self::` and `super::` references to absolute `crate::` paths.
    module_path: Vec<String>,

    /// Collected type references found in this module.
    /// All relative paths are resolved to absolute paths before being added.
    references: Vec<TypeReference>,

    /// Track if we're currently visiting inside a test module.
    /// Updated as we traverse nested modules to filter test-only references.
    in_test_module: bool,

    /// Whether to include references from test modules (those marked with `#[cfg(test)]`).
    /// When `false`, references found in test modules are excluded.
    include_tests: bool,
}

impl ModuleVisitor {
    fn new(module_name: impl Into<String>) -> Self {
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
            include_tests: false,
        }
    }

    /// Checks if a module is a test module (has #[cfg(test)] attribute).
    fn is_test_module(node: &ItemMod) -> bool {
        node.attrs.iter().any(|attr| {
            attr.path().is_ident(ATTR_CFG)
                && attr
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident == MODULE_NAME_TEST)
        })
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
            let mut reference = TypeReference::new(segments);
            reference.prefix = path_prefix;
            // Resolve relative paths to absolute paths
            reference = reference.resolve(&self.module_path);
            self.references.push(reference);
        }
    }

    /// Checks if the use tree matches the module we're interested in.
    /// Returns true if the tree starts with or references the target module.
    fn matches_module(&self, tree: &UseTree, prefix: &[String], path_prefix: &PathPrefix) -> bool {
        // If no module filter is set, allow all
        if self.module_name.is_empty() {
            return true;
        }

        let module_segments: Vec<&str> = self.module_name.split("::").collect();

        // Build the full path being checked
        let mut full_path: Vec<String> = match path_prefix {
            PathPrefix::Crate => vec![PATH_QUALIFIER_CRATE.to_string()],
            PathPrefix::SelfModule => vec![PATH_QUALIFIER_SELF.to_string()],
            PathPrefix::Super(n) => vec![PATH_QUALIFIER_SUPER.to_string(); *n],
            PathPrefix::None => Vec::new(),
        };
        full_path.extend(prefix.iter().cloned());

        // Get the first segment of the use tree
        let first_segment = Self::get_first_segment(tree);
        if let Some(seg) = first_segment {
            full_path.push(seg);
        }

        // Check if the path starts with or matches the module
        if full_path.is_empty() {
            return true;
        }

        // Check if the use path starts with the module name
        for (i, module_seg) in module_segments.iter().enumerate() {
            if i >= full_path.len() {
                // Module path is longer than use path, could be a prefix match
                return true;
            }
            if full_path[i] != *module_seg {
                return false;
            }
        }

        true
    }

    /// Gets the first segment identifier from a UseTree.
    fn get_first_segment(tree: &UseTree) -> Option<String> {
        match tree {
            UseTree::Path(p) => Some(p.ident.to_string()),
            UseTree::Name(n) => Some(n.ident.to_string()),
            UseTree::Rename(r) => Some(r.ident.to_string()),
            UseTree::Glob(_) | UseTree::Group(_) => None,
        }
    }

    fn process_use_tree(&mut self, tree: &UseTree, prefix: Vec<String>, path_prefix: PathPrefix) {
        // Check if the use tree matches the module we're interested in
        // if !self.matches_module(tree, &prefix, &path_prefix) {
        //     return;
        // }

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

                let mut reference = TypeReference::new(segments);
                reference.prefix = path_prefix;
                // Resolve relative paths to absolute paths
                reference = reference.resolve(&self.module_path);
                self.references.push(reference);
            }

            UseTree::Rename(r) => {
                let mut segments = prefix;
                segments.push(r.ident.to_string());

                let mut reference = TypeReference::new(segments);
                reference.prefix = path_prefix;
                reference.suffix = PathSuffix::Alias(r.rename.to_string());
                // Resolve relative paths to absolute paths
                reference = reference.resolve(&self.module_path);
                self.references.push(reference);
            }

            UseTree::Glob(_) => {
                let mut reference = TypeReference::new(prefix);
                reference.prefix = path_prefix;
                reference.suffix = PathSuffix::Glob;
                // Resolve relative paths to absolute paths
                reference = reference.resolve(&self.module_path);
                self.references.push(reference);
            }

            UseTree::Group(g) => {
                let group_items = self.convert_group(&g.items);

                let mut reference = TypeReference::new(prefix);
                reference.prefix = path_prefix;
                reference.suffix = PathSuffix::Group(group_items);
                // Resolve relative paths to absolute paths
                reference = reference.resolve(&self.module_path);
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
        if !self.include_tests && Self::is_test_module(i) {
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
        if !self.in_test_module || self.include_tests {
            self.process_use_tree(&node.tree, Vec::new(), PathPrefix::None);
        }
    }

    /// Visit expression paths - captures paths in expressions like `crate::foo::bar()`
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }
        syn::visit::visit_expr_path(self, node);
    }

    /// Visit type paths - captures type annotations like `let x: crate::Foo`
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if !self.in_test_module || self.include_tests {
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
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }
        syn::visit::visit_pat_struct(self, node);
    }

    /// Visit pattern tuple structs - captures tuple struct patterns
    fn visit_pat_tuple_struct(&mut self, node: &'ast syn::PatTupleStruct) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }
        syn::visit::visit_pat_tuple_struct(self, node);
    }

    /// Visit struct expressions - captures struct literal construction
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }
        syn::visit::visit_expr_struct(self, node);
    }

    /// Visit trait bounds - captures trait bounds in generics
    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }
        syn::visit::visit_trait_bound(self, node);
    }

    /// Visit impl items - captures impl blocks
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if !self.in_test_module || self.include_tests {
            // Check the trait being implemented (if any)
            if let Some((_, trait_path, _)) = &node.trait_ {
                self.process_path(trait_path);
            }
        }
        syn::visit::visit_item_impl(self, node);
    }

    /// Visit macro invocations - captures macro paths
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if !self.in_test_module || self.include_tests {
            self.process_path(&node.path);
        }
        syn::visit::visit_macro(self, node);
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;

    fn parse_use(code: &str) -> Vec<TypeReference> {
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("");
        visitor.visit_file(&syntax);
        visitor.references
    }

    #[test]
    fn test_simple_use() {
        let refs = parse_use("use std::collections::HashMap;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "std::collections::HashMap");
    }

    #[test]
    fn test_use_alias() {
        let refs = parse_use("use std::collections::HashMap as Map;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "std::collections::HashMap as Map");
    }

    #[test]
    fn test_use_glob() {
        let refs = parse_use("use std::collections::*;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "std::collections::*");
        assert!(refs[0].has_glob());
    }

    #[test]
    fn test_use_crate() {
        let refs = parse_use("use crate::module::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "crate::module::Type");
        assert!(refs[0].is_relative());
    }

    #[test]
    fn test_use_self() {
        // self:: at crate root resolves to crate::
        let refs = parse_use("use self::submodule::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "crate::submodule::Type");
        assert_eq!(refs[0].prefix, PathPrefix::Crate);
    }

    #[test]
    fn test_use_super() {
        // super:: at crate root cannot be resolved (invalid), stays as super::
        let refs = parse_use("use super::sibling::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "super::sibling::Type");
        assert_eq!(refs[0].prefix, PathPrefix::Super(1));
    }

    #[test]
    fn test_use_super_multiple() {
        // super::super:: at crate root cannot be resolved, stays as super::super::
        let refs = parse_use("use super::super::ancestor::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "super::super::ancestor::Type");
        assert_eq!(refs[0].prefix, PathPrefix::Super(2));
    }

    #[test]
    fn test_use_group() {
        let refs = parse_use("use std::collections::{HashMap, HashSet};");
        assert_eq!(refs.len(), 1);
        assert!(refs[0].has_group());
        assert_eq!(
            refs[0].to_path_string(),
            "std::collections::{HashMap, HashSet}"
        );
    }

    #[test]
    fn test_use_group_with_self() {
        let refs = parse_use("use std::collections::{self, HashMap};");
        assert_eq!(refs.len(), 1);
        assert!(refs[0].has_group());
    }

    #[test]
    fn test_use_nested_group() {
        let refs = parse_use("use std::{collections::{HashMap, HashSet}, io::Read};");
        assert_eq!(refs.len(), 1);
        assert!(refs[0].has_group());
    }

    #[test]
    fn test_crate_analyzer() {
        let analyzer = CrateAnalyzer::new("test_crate");
        assert_eq!(analyzer.crate_name(), "test_crate");
        assert_eq!(analyzer.file_count(), 0);
        assert_eq!(analyzer.total_references(), 0);
    }

    #[test]
    fn test_type_path_collection() {
        let code = "
            fn foo(x: crate::MyType) -> crate::Result {
                let y: crate::Other = x;
                y
            }
        ";
        let refs = parse_use(code);
        assert!(refs.len() >= 2, "Should capture type annotations");

        // Should capture both MyType and Result (and possibly Other)
        let paths: Vec<String> = refs.iter().map(TypeReference::to_path_string).collect();
        assert!(paths.iter().any(|p| p.contains("MyType")));
        assert!(paths.iter().any(|p| p.contains("Result")));
    }

    #[test]
    fn test_expr_path_collection() {
        let code = "
            fn foo() {
                crate::module::function();
                let x = crate::module::Type::new();
            }
        ";
        let refs = parse_use(code);
        assert!(refs.len() >= 2, "Should capture expression paths");

        let paths: Vec<String> = refs.iter().map(TypeReference::to_path_string).collect();
        assert!(paths.iter().any(|p| p.contains("function")));
        assert!(paths.iter().any(|p| p.contains("Type")));
    }

    #[test]
    fn test_impl_trait_collection() {
        let code = "
            impl crate::MyTrait for Foo {
                fn bar() {}
            }
        ";
        let refs = parse_use(code);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "crate::MyTrait");
    }

    #[test]
    fn test_struct_pattern_collection() {
        let code = "
            fn foo(x: Something) {
                match x {
                    crate::module::Variant { field } => field,
                }
            }
        ";
        let refs = parse_use(code);
        assert!(refs.iter().any(|r| r.to_path_string().contains("Variant")));
    }

    #[test]
    fn test_macro_path_collection() {
        let code = "
            fn foo() {
                crate::macros::my_macro!();
            }
        ";
        let refs = parse_use(code);
        assert_eq!(refs.len(), 1);
        assert!(refs[0].to_path_string().contains("my_macro"));
    }

    #[test]
    fn test_resolve_self_in_module() {
        // Test resolution of self:: in a nested module
        let code = "use self::submodule::Type;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils::parser");
        visitor.visit_file(&syntax);

        assert_eq!(visitor.references.len(), 1);
        assert_eq!(
            visitor.references[0].to_path_string(),
            "crate::utils::parser::submodule::Type"
        );
        assert_eq!(visitor.references[0].prefix, PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_super_in_nested_module() {
        // Test resolution of super:: in a nested module
        let code = "use super::sibling::Type;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils::parser");
        visitor.visit_file(&syntax);

        assert_eq!(visitor.references.len(), 1);
        assert_eq!(
            visitor.references[0].to_path_string(),
            "crate::utils::sibling::Type"
        );
        assert_eq!(visitor.references[0].prefix, PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_super_multiple_in_deeply_nested_module() {
        // Test resolution of super::super:: in a deeply nested module
        let code = "use super::super::ancestor::Type;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("a::b::c");
        visitor.visit_file(&syntax);

        assert_eq!(visitor.references.len(), 1);
        assert_eq!(
            visitor.references[0].to_path_string(),
            "crate::a::ancestor::Type"
        );
        assert_eq!(visitor.references[0].prefix, PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_preserves_groups() {
        // Test that resolution works with grouped imports
        let code = "use self::{foo, bar::Baz};";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils");
        visitor.visit_file(&syntax);

        assert_eq!(visitor.references.len(), 1);
        assert_eq!(
            visitor.references[0].to_path_string(),
            "crate::utils::{foo, bar::Baz}"
        );
        assert_eq!(visitor.references[0].prefix, PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_preserves_glob() {
        // Test that resolution works with glob imports
        let code = "use self::submodule::*;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils");
        visitor.visit_file(&syntax);

        assert_eq!(visitor.references.len(), 1);
        assert_eq!(
            visitor.references[0].to_path_string(),
            "crate::utils::submodule::*"
        );
        assert_eq!(visitor.references[0].prefix, PathPrefix::Crate);
        assert!(visitor.references[0].has_glob());
    }

    #[test]
    fn test_resolve_preserves_alias() {
        // Test that resolution works with aliased imports
        let code = "use self::submodule::Type as MyType;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils");
        visitor.visit_file(&syntax);

        assert_eq!(visitor.references.len(), 1);
        assert_eq!(
            visitor.references[0].to_path_string(),
            "crate::utils::submodule::Type as MyType"
        );
        assert_eq!(visitor.references[0].prefix, PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_expression_paths() {
        // Test that resolution works for paths in expressions
        let code = "
            fn foo() {
                self::helper::do_something();
                super::sibling::bar();
            }
        ";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils::parser");
        visitor.visit_file(&syntax);

        assert!(visitor.references.len() >= 2);

        // Check that paths are resolved
        let paths: Vec<String> = visitor
            .references
            .iter()
            .map(TypeReference::to_path_string)
            .collect();

        assert!(
            paths
                .iter()
                .any(|p| p.contains("crate::utils::parser::helper"))
        );
        assert!(paths.iter().any(|p| p.contains("crate::utils::sibling")));
    }
}
