#![allow(dead_code)]
//! Module for parsing Rust source files and extracting type references.
//!
//! Provides [`CrateAnalyzer`] for collecting [`TypeReference`]s from multiple
//! source files within a crate.

mod visitor;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use syn::File;
use syn::visit::Visit;
use thiserror::Error;

use crate::reference::TypeReference;
use visitor::ModuleVisitor;

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
    ///
    /// When `inline_scope` is non-empty, the visitor is scoped to only the items
    /// inside the target inline module instead of visiting the entire file.
    /// For example, if parsing `glob_patterns::utilities` from `glob_patterns.rs`,
    /// `inline_scope` would be `["utilities"]`.
    pub fn parse_file(
        &mut self,
        module: impl Into<String>,
        path: &Path,
        inline_scope: &[String],
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

        if inline_scope.is_empty() {
            visitor.visit_file(&syntax);
        } else if let Some(items) = find_inline_items(&syntax.items, inline_scope) {
            for item in items {
                visitor.visit_item(item);
            }
        }

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

/// Navigate the AST to find items inside a nested inline module.
///
/// Given a list of items and a scope path like `["utilities"]`, descends into the
/// `mod utilities { ... }` item and returns its items. For deeper scopes like
/// `["a", "b"]`, it descends recursively: first into `mod a`, then into `mod b`.
///
/// Returns `None` if the scope is empty or if the target inline module is not found.
fn find_inline_items<'a>(items: &'a [syn::Item], scope: &[String]) -> Option<&'a Vec<syn::Item>> {
    if scope.is_empty() {
        return None;
    }
    let target = &scope[0];
    for item in items {
        if let syn::Item::Mod(item_mod) = item
            && item_mod.ident == *target
            && let Some((_, nested)) = &item_mod.content
        {
            if scope.len() == 1 {
                return Some(nested);
            }
            return find_inline_items(nested, &scope[1..]);
        }
    }
    None
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::PathPrefix;

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
