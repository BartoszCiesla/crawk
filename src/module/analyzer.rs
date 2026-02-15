//! Module for parsing Rust source files and extracting type references.
//!
//! Provides [`CrateAnalyzer`] for collecting [`TypeReference`]s from multiple
//! source files within a crate.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{File, ItemUse, UseTree};
use thiserror::Error;

use super::path::{GroupItem, PathPrefix, TypeReference};

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
///
/// # Example
///
/// ```no_run
/// use crawk::module::analyzer::CrateAnalyzer;
/// use std::path::Path;
///
/// let mut analyzer = CrateAnalyzer::new("my_crate");
///
/// // Parse source files
/// analyzer.parse_file("module", Path::new("src/module/mod.rs"))?;
/// analyzer.parse_file("module::submodule", Path::new("src/module/submodule.rs"))?;
///
/// // Get all references
/// for (file, refs) in analyzer.all_references() {
///     println!("{file}: {} references", refs.len());
/// }
///
/// // Get flattened list of all references
/// let all_refs: Vec<_> = analyzer.iter_references().collect();
/// # Ok::<(), crawk::module::analyzer::AnalyzerError>(())
/// ```
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
    pub fn parse_file(&mut self, module: impl Into<String>, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path).map_err(|e| AnalyzerError::FileRead {
            path: path.to_path_buf(),
            source: e,
        })?;

        let syntax: File = syn::parse_file(&content).map_err(|e| AnalyzerError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        let mut visitor = ModuleVisitor::new();
        visitor.visit_file(&syntax);

        let mut file_refs = FileReferences::new(path);
        file_refs.references = visitor.references;

        let module = module.into();
        if !self.files.contains_key(&module) {
            self.file_order.push(module.clone());
        }
        self.files.insert(module, file_refs);

        Ok(())
    }

    /// Returns all collected references by module, in parse order.
    pub fn all_references(&self) -> impl Iterator<Item = (&String, &FileReferences)> {
        self.file_order
            .iter()
            .filter_map(|module| self.files.get(module).map(|refs| (module, refs)))
    }

    /// Returns an iterator over all references across all files.
    pub fn iter_references(&self) -> impl Iterator<Item = &TypeReference> {
        self.files.values().flat_map(|f| f.references.iter())
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
    references: Vec<TypeReference>,
}

impl ModuleVisitor {
    const fn new() -> Self {
        Self {
            references: Vec::new(),
        }
    }

    fn process_use_tree(&mut self, tree: &UseTree, prefix: Vec<String>, path_prefix: PathPrefix) {
        match tree {
            UseTree::Path(p) => {
                let ident = p.ident.to_string();

                // Check for special prefixes at the start
                let (new_prefix, new_path_prefix) = if prefix.is_empty() {
                    match ident.as_str() {
                        "crate" => (Vec::new(), PathPrefix::Crate),
                        "self" => (Vec::new(), PathPrefix::SelfModule),
                        "super" => {
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
                } else if ident == "super" {
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
                self.references.push(reference);
            }

            UseTree::Rename(r) => {
                let mut segments = prefix;
                segments.push(r.ident.to_string());

                let mut reference = TypeReference::new(segments);
                reference.prefix = path_prefix;
                reference.alias = Some(r.rename.to_string());
                self.references.push(reference);
            }

            UseTree::Glob(_) => {
                let mut reference = TypeReference::new(prefix);
                reference.prefix = path_prefix;
                reference.is_glob = true;
                self.references.push(reference);
            }

            UseTree::Group(g) => {
                let group_items = self.convert_group(&g.items);

                let mut reference = TypeReference::new(prefix);
                reference.prefix = path_prefix;
                reference.group = Some(group_items);
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
                if ident == "self" {
                    GroupItem::SelfItem { alias: None }
                } else {
                    GroupItem::Simple(ident)
                }
            }

            UseTree::Rename(r) => {
                let ident = r.ident.to_string();
                let alias = r.rename.to_string();
                if ident == "self" {
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
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        self.process_use_tree(&node.tree, Vec::new(), PathPrefix::None);
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;

    fn parse_use(code: &str) -> Vec<TypeReference> {
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new();
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
        let refs = parse_use("use self::submodule::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "self::submodule::Type");
    }

    #[test]
    fn test_use_super() {
        let refs = parse_use("use super::sibling::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "super::sibling::Type");
    }

    #[test]
    fn test_use_super_multiple() {
        let refs = parse_use("use super::super::ancestor::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "super::super::ancestor::Type");
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
}
