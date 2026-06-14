//! Module for parsing Rust source files and extracting type references.
//!
//! Provides [`CrateAnalyzer`] for collecting [`TypeReference`]s from multiple
//! source files within a crate.

mod visitor;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::cache::ParseCache;
use crate::utils::{ReadFileError, descend_inline_module, read_source_file};

use syn::File;
use syn::visit::Visit;
use thiserror::Error;

use tracing::info;

use crate::reference::{PathPrefix, TypeReference};
use visitor::ModuleVisitor;

/// Errors that can occur while reading or parsing a Rust source file.
///
/// Returned as the `source` field of [`AnalysisError::ModuleAnalysisFailed`](crate::AnalysisError::ModuleAnalysisFailed).
#[derive(Debug, Error)]
pub(crate) enum AnalyzerError {
    /// The source file could not be read from disk.
    #[error("Failed to read file '{path}': {source}")]
    FileRead {
        /// Path to the file that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The source file exceeds the maximum allowed size and was not parsed.
    ///
    /// This limit exists to prevent excessive memory usage on unexpectedly large files.
    #[error("File too large '{path}': {size} bytes (limit {limit} bytes)")]
    FileTooLarge {
        /// Path to the oversized file.
        path: PathBuf,
        /// Actual file size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        limit: u64,
    },

    /// The source file could not be parsed as valid Rust syntax.
    #[error("Failed to parse file '{path}': {message}")]
    Parse {
        /// Path to the file that failed to parse.
        path: PathBuf,
        /// Description of the parse error from `syn`.
        message: String,
    },
}

/// Result type for analyzer operations.
pub(crate) type Result<T> = std::result::Result<T, AnalyzerError>;

/// Analyzer for collecting type references from a Rust crate.
#[derive(Debug, Clone)]
pub(crate) struct CrateAnalyzer {
    /// Name of the crate being analyzed.
    crate_name: String,

    /// Collected references per module path.
    files: HashMap<String, Vec<TypeReference>>,

    /// Order in which files were parsed (for deterministic iteration).
    file_order: Vec<String>,
}

impl CrateAnalyzer {
    /// Creates a new analyzer for the given crate.
    pub(crate) fn new(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            files: HashMap::new(),
            file_order: Vec::new(),
        }
    }

    /// Parses a single source file and collects type references.
    ///
    /// When `inline_scope` is non-empty, the visitor is scoped to only the items
    /// inside the target inline module instead of visiting the entire file.
    /// For example, if parsing `glob_patterns::utilities` from `glob_patterns.rs`,
    /// `inline_scope` would be `["utilities"]`.
    pub(crate) fn parse_file(
        &mut self,
        module: impl Into<String>,
        path: &Path,
        inline_scope: &[String],
        children: HashSet<String>,
        cache: &mut ParseCache,
    ) -> Result<Vec<TypeReference>> {
        let syntax: Rc<File> = cache.get_or_parse(path, |p| {
            let content = read_source_file(p).map_err(|e| match e {
                ReadFileError::Io(source) => AnalyzerError::FileRead {
                    path: p.to_path_buf(),
                    source,
                },
                ReadFileError::TooLarge { size, limit } => AnalyzerError::FileTooLarge {
                    path: p.to_path_buf(),
                    size,
                    limit,
                },
            })?;
            syn::parse_file(&content).map_err(|e| AnalyzerError::Parse {
                path: p.to_path_buf(),
                message: e.to_string(),
            })
        })?;

        let module = module.into();
        let package_name = Some(self.crate_name.clone());
        let mut visitor = ModuleVisitor::new(module.clone(), children, package_name);

        if inline_scope.is_empty() {
            visitor.visit_file(&syntax);
        } else if let Some(items) = descend_inline_module(&syntax.items, inline_scope) {
            for item in items {
                visitor.visit_item(item);
            }
        }

        let result: Vec<TypeReference> = visitor.references.all().cloned().collect();
        info!(
            "Parsed '{module}': {} references from {}{}",
            result.len(),
            path.display(),
            if inline_scope.is_empty() {
                String::new()
            } else {
                format!(" (inline {inline_scope:?})")
            }
        );

        if !self.files.contains_key(&module) {
            self.file_order.push(module.clone());
        }
        self.files.insert(module, result.clone());

        Ok(result)
    }

    /// Returns all collected crate internal references by module, in parse order.
    ///
    /// `children_map` maps each module path to the set of its direct child module
    /// names. This allows bare `use child::Item` paths (valid in Rust ≥2018 for
    /// direct children declared via `mod`) to be recognised as internal references.
    ///
    /// When `module_filter` is `Some`, only modules in that set are yielded. This
    /// scopes a single `analyze_module` call to its target's modules and prevents
    /// refs parsed for an earlier target (e.g. lib) from leaking into the result
    /// of a later, narrower analysis (e.g. a test target). When `None`, every
    /// module ever parsed by this analyzer is yielded.
    pub(crate) fn all_crate_references<'a>(
        &'a self,
        children_map: &'a HashMap<String, HashSet<String>>,
        module_filter: Option<&'a HashSet<String>>,
    ) -> impl Iterator<Item = (&'a String, Vec<&'a TypeReference>)> {
        self.file_order
            .iter()
            .filter(move |module| module_filter.is_none_or(|f| f.contains(module.as_str())))
            .filter_map(move |module| {
                self.files.get(module).map(|refs| {
                    let crate_refs: Vec<&TypeReference> = refs
                        .iter()
                        .filter(|r| {
                            r.is_relative()
                                || r.is_from_crate(&self.crate_name)
                                || Self::is_bare_child(r, module, children_map)
                        })
                        .collect();
                    (module, crate_refs)
                })
            })
    }

    /// Check if a `PathPrefix::None` reference targets an internal module.
    ///
    /// Two resolution rules are checked:
    /// - **Edition 2018+**: bare `use child::Item` resolves to a direct child
    ///   declared via `mod child;` in the current module.
    /// - **Edition 2015**: bare `use sibling::Item` resolves from the crate
    ///   root, so any top-level module is reachable from anywhere.
    ///
    /// Both are covered by checking `children_map[module]` (direct children)
    /// and `children_map[""]` (crate-root children / top-level modules).
    fn is_bare_child(
        r: &TypeReference,
        module: &str,
        children_map: &HashMap<String, HashSet<String>>,
    ) -> bool {
        if r.prefix() != PathPrefix::None {
            return false;
        }
        let first = r.segments().first();
        first.is_some_and(|s| {
            children_map
                .get(module)
                .is_some_and(|ch| ch.contains(s.as_str()))
                || children_map
                    .get("")
                    .is_some_and(|ch| ch.contains(s.as_str()))
        })
    }
}

#[cfg(test)]
impl CrateAnalyzer {
    pub(crate) fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub(crate) fn total_references(&self) -> usize {
        self.files.values().map(Vec::len).sum()
    }

    pub(crate) fn file_count(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::PathPrefix;
    use std::io::Write;
    use std::path::Path;
    use tempfile::NamedTempFile;

    fn parse_use(code: &str) -> Vec<TypeReference> {
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("", HashSet::new(), None);
        visitor.visit_file(&syntax);
        visitor.references.all().cloned().collect()
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
        assert_eq!(refs[0].prefix(), PathPrefix::Crate);
    }

    #[test]
    fn test_use_super() {
        // super:: at crate root cannot be resolved (invalid), stays as super::
        let refs = parse_use("use super::sibling::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "super::sibling::Type");
        assert_eq!(refs[0].prefix(), PathPrefix::Super(1));
    }

    #[test]
    fn test_use_super_multiple() {
        // super::super:: at crate root cannot be resolved, stays as super::super::
        let refs = parse_use("use super::super::ancestor::Type;");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].to_path_string(), "super::super::ancestor::Type");
        assert_eq!(refs[0].prefix(), PathPrefix::Super(2));
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
    fn test_use_group_path_items_produce_nested_with_items() {
        // Regression: `client::ClientAction` inside a group must produce
        // Nested { prefix: ["client"], items: [Simple("ClientAction")] }
        // so that expand_groups can expand it correctly.
        use crate::analyzer::expand_groups;
        use crate::reference::GroupItem;

        let refs = parse_use(
            "use crate::args::{client::ClientAction, system::{PingArgs, StatsArgs}, topic::TopicAction};",
        );
        assert_eq!(refs.len(), 1);
        assert!(refs[0].has_group());

        let expanded = expand_groups(&refs[0]);
        let paths: Vec<String> = expanded.iter().map(TypeReference::to_path_string).collect();

        assert!(
            paths.contains(&"crate::args::client::ClientAction".to_owned()),
            "client::ClientAction missing from expanded: {paths:?}"
        );
        assert!(
            paths.contains(&"crate::args::topic::TopicAction".to_owned()),
            "topic::TopicAction missing from expanded: {paths:?}"
        );
        assert!(
            paths.contains(&"crate::args::system::PingArgs".to_owned()),
            "system::PingArgs missing from expanded: {paths:?}"
        );
        assert!(
            paths.contains(&"crate::args::system::StatsArgs".to_owned()),
            "system::StatsArgs missing from expanded: {paths:?}"
        );
        assert_eq!(expanded.len(), 4);

        // Verify the group items are Nested with non-empty items
        if let crate::reference::PathSuffix::Group(items) = refs[0].suffix() {
            for item in items {
                if let GroupItem::Nested { prefix, items } = item {
                    assert!(
                        !items.is_empty(),
                        "Nested group item {prefix:?} has empty items"
                    );
                }
            }
        }
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
        let mut visitor = ModuleVisitor::new("utils::parser", HashSet::new(), None);
        visitor.visit_file(&syntax);

        let uses = &visitor.references.use_statements;
        assert_eq!(uses.len(), 1);
        assert_eq!(
            uses[0].to_path_string(),
            "crate::utils::parser::submodule::Type"
        );
        assert_eq!(uses[0].prefix(), PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_super_in_nested_module() {
        // Test resolution of super:: in a nested module
        let code = "use super::sibling::Type;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils::parser", HashSet::new(), None);
        visitor.visit_file(&syntax);

        let uses = &visitor.references.use_statements;
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].to_path_string(), "crate::utils::sibling::Type");
        assert_eq!(uses[0].prefix(), PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_super_multiple_in_deeply_nested_module() {
        // Test resolution of super::super:: in a deeply nested module
        let code = "use super::super::ancestor::Type;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("a::b::c", HashSet::new(), None);
        visitor.visit_file(&syntax);

        let uses = &visitor.references.use_statements;
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].to_path_string(), "crate::a::ancestor::Type");
        assert_eq!(uses[0].prefix(), PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_preserves_groups() {
        // Test that resolution works with grouped imports
        let code = "use self::{foo, bar::Baz};";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils", HashSet::new(), None);
        visitor.visit_file(&syntax);

        let uses = &visitor.references.use_statements;
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].to_path_string(), "crate::utils::{foo, bar::Baz}");
        assert_eq!(uses[0].prefix(), PathPrefix::Crate);
    }

    #[test]
    fn test_resolve_preserves_glob() {
        // Test that resolution works with glob imports
        let code = "use self::submodule::*;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils", HashSet::new(), None);
        visitor.visit_file(&syntax);

        let uses = &visitor.references.use_statements;
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].to_path_string(), "crate::utils::submodule::*");
        assert_eq!(uses[0].prefix(), PathPrefix::Crate);
        assert!(uses[0].has_glob());
    }

    #[test]
    fn test_resolve_preserves_alias() {
        // Test that resolution works with aliased imports
        let code = "use self::submodule::Type as MyType;";
        let syntax: File = syn::parse_file(code).unwrap();
        let mut visitor = ModuleVisitor::new("utils", HashSet::new(), None);
        visitor.visit_file(&syntax);

        let uses = &visitor.references.use_statements;
        assert_eq!(uses.len(), 1);
        assert_eq!(
            uses[0].to_path_string(),
            "crate::utils::submodule::Type as MyType"
        );
        assert_eq!(uses[0].prefix(), PathPrefix::Crate);
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
        let mut visitor = ModuleVisitor::new("utils::parser", HashSet::new(), None);
        visitor.visit_file(&syntax);

        assert!(visitor.references.value_refs.len() >= 2);

        // Check that paths are resolved
        let paths: Vec<String> = visitor
            .references
            .value_refs
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

    #[test]
    fn parse_file_returns_file_read_error_for_nonexistent_file() {
        let mut analyzer = CrateAnalyzer::new("test");
        let mut cache = ParseCache::new();
        let err = analyzer
            .parse_file(
                "mod",
                Path::new("/nonexistent/file.rs"),
                &[],
                HashSet::new(),
                &mut cache,
            )
            .unwrap_err();
        assert!(matches!(err, AnalyzerError::FileRead { .. }));
    }

    #[test]
    fn parse_file_returns_parse_error_for_invalid_syntax() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "this is not valid rust !!!").unwrap();
        let mut analyzer = CrateAnalyzer::new("test");
        let mut cache = ParseCache::new();
        let err = analyzer
            .parse_file("mod", f.path(), &[], HashSet::new(), &mut cache)
            .unwrap_err();
        assert!(matches!(err, AnalyzerError::Parse { .. }));
    }

    #[test]
    fn read_source_file_returns_file_too_large_when_size_exceeds_limit() {
        use crate::utils::{MAX_FILE_BYTES, ReadFileError};
        use std::io::Write;
        let mut f = NamedTempFile::new().unwrap();
        let chunk = vec![b' '; 1024];
        for _ in 0..=(MAX_FILE_BYTES / 1024) {
            f.write_all(&chunk).unwrap();
        }
        f.flush().unwrap();
        let err = read_source_file(f.path()).unwrap_err();
        assert!(matches!(err, ReadFileError::TooLarge { limit, .. } if limit == MAX_FILE_BYTES));
    }
}
