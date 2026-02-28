//! crawk - Rust module dependency analyzer
//!
//! This library provides tools for analyzing module dependencies in Rust codebases.
//! It parses Rust source code to identify all internal crate references including
//! `use` statements, type annotations, trait bounds, struct literals, and macro invocations.
//!
//! # Quick Start
//!
//! ```no_run
//! use crawk::{Analyzer, AnalysisOptions};
//! use std::path::Path;
//!
//! let mut analyzer = Analyzer::new(Path::new("/path/to/crate"))?;
//! let options = AnalysisOptions::default();
//! let result = analyzer.analyze_module("utils::parser", &options)?;
//!
//! for (module, refs) in result.dependencies() {
//!     println!("{module}");
//!     for reference in refs {
//!         println!("  {reference}");
//!     }
//! }
//! # Ok::<(), crawk::AnalysisError>(())
//! ```
//!
//! # Features
//!
//! - **Comprehensive dependency detection**: Captures not just `use` statements but also
//!   type annotations, trait bounds, struct patterns/literals, impl blocks, and macro invocations
//! - **Path expansion**: Resolves `self::` and `super::` to absolute `crate::` paths
//! - **Glob expansion**: Optionally expands `use crate::foo::*` to explicit items
//! - **Test module filtering**: Optionally include or exclude `#[cfg(test)]` modules
//! - **Depth limiting**: Truncate [`TypeReference`] paths via [`TypeReference::truncate_to_depth`]

use crate::module::analyzer::{AnalyzerError, CrateAnalyzer};
use crate::module::discover::{CrateInfo, CrateInfoError};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, error, info, trace};

mod constants;
mod module;
pub mod version;

pub use crate::module::path::{GroupItem, PathPrefix, PathSuffix, Segments, TypeReference};

/// Options for dependency analysis.
///
/// Controls how the analyzer processes modules and formats output.
///
/// # Examples
///
/// ```
/// use crawk::AnalysisOptions;
///
/// // Default options: exclude tests, don't expand groups, don't resolve globs
/// let options = AnalysisOptions::default();
///
/// // Include test modules and expand grouped imports
/// let options = AnalysisOptions {
///     include_tests: true,
///     expand_groups: true,
///     ..Default::default()
/// };
///
/// // Resolve glob imports to explicit items
/// let options = AnalysisOptions {
///     resolve_globs: true,
///     ..Default::default()
/// };
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct AnalysisOptions {
    /// Recursively analyze all submodules of the specified module.
    ///
    /// When `false` (default), only the specified module is analyzed.
    /// When `true`, all nested submodules are also analyzed. For example,
    /// if analyzing `foo` with `recursive = true`, it will analyze
    /// `foo`, `foo::bar`, `foo::baz`, etc.
    pub recursive: bool,

    /// Include test modules (`#[cfg(test)]`) in analysis.
    ///
    /// When `false` (default), dependencies from test modules are excluded.
    pub include_tests: bool,

    /// Expand grouped imports into individual paths.
    ///
    /// When `true`, `use crate::foo::{Bar, Baz}` becomes two separate entries:
    /// `foo::Bar` and `foo::Baz`.
    pub expand_groups: bool,

    /// Resolve glob imports to explicit items.
    ///
    /// When `true`, `use crate::foo::*` is expanded into the individual public
    /// items exported by module `foo` (e.g., `foo::Bar`, `foo::Baz`).
    pub resolve_globs: bool,
}

/// Result of analyzing a module's dependencies.
///
/// Contains the set of internal crate dependencies found in the analyzed module
/// and all its submodules.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// The analyzed module path (e.g., `"utils::parser"`).
    module_path: String,

    /// Set of internal dependencies found for the analyzed modules.
    dependencies: HashMap<String, HashSet<TypeReference>>,

    /// Path to the source file that was analyzed.
    source_file: PathBuf,
}

impl AnalysisResult {
    /// Returns the analyzed module path.
    #[must_use]
    pub const fn module_path(&self) -> &String {
        &self.module_path
    }

    /// Returns the set of dependencies found.
    #[must_use]
    pub const fn dependencies(&self) -> &HashMap<String, HashSet<TypeReference>> {
        &self.dependencies
    }

    /// Returns the path to the analyzed source file.
    #[must_use]
    pub fn source_file(&self) -> &Path {
        &self.source_file
    }

    /// Returns `true` if no dependencies were found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dependencies.is_empty()
    }

    /// Returns the number of dependencies found.
    #[must_use]
    pub fn len(&self) -> usize {
        self.dependencies.len()
    }

    /// Consumes the result and returns the dependencies as a sorted vector.
    #[must_use]
    pub fn into_sorted_vec(self) -> Vec<TypeReference> {
        let all_deps_unique: HashSet<_> = self.dependencies.values().flatten().cloned().collect();
        let mut all_deps_unique: Vec<TypeReference> = all_deps_unique.into_iter().collect();
        all_deps_unique.sort_by_key(TypeReference::to_path_string);
        all_deps_unique
    }
}

/// Error types for analysis operations.
#[derive(Debug, Error)]
pub enum AnalysisError {
    /// The specified module was not found in the crate.
    #[error("Module not found: {module_path}")]
    ModuleNotFound {
        /// The module path that was not found.
        module_path: String,
    },

    /// The crate root directory does not exist or is not a valid Rust project.
    #[error("Invalid crate root: {path} - {reason}")]
    InvalidCrateRoot {
        /// The path that was provided.
        path: PathBuf,
        /// Description of what's wrong.
        reason: String,
    },

    /// Errors related to crate metadata retrieval and validation.
    #[error(transparent)]
    CrateInfoError(#[from] CrateInfoError),

    /// Errors that occur during module parsing and analysis.
    #[error("Error analyzing module: {0}")]
    AnalyzerError(#[from] AnalyzerError),
}

/// Result type alias for analysis info operations.
pub type Result<T> = std::result::Result<T, AnalysisError>;

/// Analyzer for Rust module dependencies.
///
/// The main entry point for analyzing module dependencies in a Rust crate.
/// Create an analyzer with a crate root path, then call [`analyze_module`](Self::analyze_module)
/// to analyze specific modules.
///
/// # Examples
///
/// ```no_run
/// use crawk::{Analyzer, AnalysisOptions};
/// use std::path::Path;
///
/// let mut analyzer = Analyzer::new(Path::new("/path/to/my-crate"))?;
///
/// // Analyze the "utils" module
/// let result = analyzer.analyze_module("utils", &AnalysisOptions::default())?;
/// println!("Found {} dependencies", result.len());
///
/// // Analyze a nested module with custom options
/// let options = AnalysisOptions {
///     include_tests: true,
///     expand_groups: true,
///     ..Default::default()
/// };
/// let result = analyzer.analyze_module("foo::bar", &options)?;
/// # Ok::<(), crawk::AnalysisError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Analyzer {
    /// Crate analyzer
    crate_info: CrateInfo,
    /// Module analyzer
    crate_analyzer: CrateAnalyzer,
}

impl Analyzer {
    /// Create a new analyzer for the given crate root directory.
    ///
    /// The crate root should be the directory containing `Cargo.toml`.
    /// The analyzer will look for source files in the `src/` subdirectory.
    ///
    /// # Arguments
    ///
    /// * `crate_root` - Path to the crate root directory
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::InvalidCrateRoot`] if the path does not exist or is not a valid Rust project.
    /// Returns [`AnalysisError::CrateInfoError`] if there are issues retrieving crate metadata.
    /// Returns [`AnalysisError::AnalyzerError`] if there are issues initializing the crate analyzer.
    /// Returns `Ok(Analyzer)` if the crate root is valid and the analyzer is successfully initialized.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawk::Analyzer;
    /// use std::path::Path;
    ///
    /// let analyzer = Analyzer::new(Path::new("/home/user/my-project"));
    /// ```
    pub fn new(crate_root: impl AsRef<Path>) -> Result<Self> {
        let crate_info = CrateInfo::new(crate_root.as_ref())?;
        let name = crate_info.root_package_name();
        let crate_analyzer = CrateAnalyzer::new(name);

        Ok(Self {
            crate_info,
            crate_analyzer,
        })
    }

    // /// Find the source file for a module path.
    // ///
    // /// Returns `Some(path)` if the module exists, `None` otherwise.
    // ///
    // /// # Arguments
    // ///
    // /// * `module_path` - Module path components (e.g., `["utils", "parser"]`)
    // ///
    // /// # Examples
    // ///
    // /// ```no_run
    // /// use crawk::Analyzer;
    // /// use std::path::Path;
    // ///
    // /// let analyzer = Analyzer::new(Path::new("/path/to/crate"));
    // /// if let Some(file) = analyzer.find_module(&["utils"]) {
    // ///     println!("Found module at: {}", file.display());
    // /// }
    // /// ```
    // #[must_use]
    // pub fn find_module(&self, module_path: &[impl AsRef<str>]) -> Option<PathBuf> {
    //     let path_strings: Vec<String> =
    //         module_path.iter().map(|s| s.as_ref().to_string()).collect();
    //     module::locate::find_module_by_path(&self.src_dir, &path_strings)
    // }

    /// Analyze dependencies for a specific module.
    ///
    /// Recursively analyzes the module and all its submodules, collecting
    /// all internal crate dependencies.
    ///
    /// # Arguments
    ///
    /// * `module_path` - Module path components (e.g., `["utils", "parser"]`)
    /// * `options` - Analysis options controlling output format
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::ModuleNotFound`] if the module doesn't exist.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawk::{Analyzer, AnalysisOptions};
    /// use std::path::Path;
    ///
    /// let mut analyzer = Analyzer::new(Path::new("/path/to/crate"))?;
    /// let result = analyzer.analyze_module("utils::parser", &AnalysisOptions::default())?;
    ///
    /// for (module, refs) in result.dependencies() {
    ///     println!("{module}");
    ///     for reference in refs {
    ///         println!("  {reference}");
    ///     }
    /// }
    /// # Ok::<(), crawk::AnalysisError>(())
    /// ```
    pub fn analyze_module(
        &mut self,
        module_path: impl Into<String>,
        options: &AnalysisOptions,
    ) -> Result<AnalysisResult> {
        let module_path = module_path.into();

        let modules = self.crate_info.get_module_tree(
            &module_path,
            options.recursive,
            options.include_tests,
        )?;

        let source_file = modules
            .first()
            .map(|m| m.source().to_path_buf())
            .unwrap_or_default();

        let file_root = Self::build_file_root_map(&modules);

        for module in modules {
            let root_path = &file_root[module.source()];
            let inline_scope = Self::compute_inline_scope(module.path(), root_path);

            trace!(
                "Module '{}' inline_scope={:?} (file root: '{}')",
                module.path(),
                inline_scope,
                root_path
            );

            info!(
                "Analyzing module: {} (file: {})",
                module.path(),
                module.source().display()
            );
            match self
                .crate_analyzer
                .parse_file(module.path(), module.source(), &inline_scope)
            {
                Err(e) => {
                    error!("Error while analyzing module: {e}");
                    return Err(AnalysisError::AnalyzerError(e));
                }
                Ok(type_list) => {
                    info!("Analyzed {}", module.path());
                    for reference in &type_list {
                        debug!("Analyzed {reference:?}");
                        info!("Found reference: {}", reference.to_path_string());
                    }
                }
            }
        }

        let mut dependencies = HashMap::new();
        for (module, module_references) in self.crate_analyzer.all_crate_references() {
            debug!("Processing module: {}", module);
            let mut refs = HashSet::new();
            for reference in module_references {
                debug!("Found crate reference: {}", reference.to_path_string());

                // Pass 1: expand groups if requested
                let after_expand = if options.expand_groups {
                    debug!(
                        "Expanding groups for reference: {}",
                        reference.to_path_string()
                    );
                    let expanded = reference.expand_suffix();
                    for exp in &expanded {
                        debug!("Expanded reference: {}", exp.to_path_string());
                    }
                    expanded
                } else {
                    vec![reference.clone()]
                };

                // Pass 2: resolve globs if requested
                for r in after_expand {
                    if options.resolve_globs && r.has_glob() {
                        debug!("Resolving glob: {}", r.to_path_string());
                        let resolved = self.resolve_glob(&r);
                        for res in resolved {
                            debug!("Resolved glob item: {}", res.to_path_string());
                            refs.insert(res);
                        }
                    } else {
                        refs.insert(r);
                    }
                }
            }

            debug!(
                "Processing module: {module} complete, found {} dependencies",
                dependencies.len()
            );
            dependencies.insert(module.clone(), refs);
        }

        Ok(AnalysisResult {
            module_path,
            dependencies,
            source_file,
        })
    }

    /// Resolve a glob `TypeReference` (e.g., `crate::foo::bar::*`) into concrete
    /// references by reading the target module's public API.
    ///
    /// Only `crate::` prefixed globs are resolved. Other prefixes pass through
    /// unchanged. If the module file cannot be found or parsed, the original
    /// glob reference is returned with a warning.
    fn resolve_glob(&self, reference: &TypeReference) -> Vec<TypeReference> {
        use crate::module::resolve::extract_public_items;

        // Determine the module path to resolve.
        // Accept both `crate::foo::bar::*` (PathPrefix::Crate, segments=["foo","bar"])
        // and `mycrate::foo::bar::*` (PathPrefix::None, first segment == crate name).
        let is_crate_prefix = reference.prefix == PathPrefix::Crate;
        let is_crate_name_prefix = reference.prefix == PathPrefix::None
            && reference
                .segments
                .first()
                .is_some_and(|s| s == self.crate_info.root_package_name());

        if !is_crate_prefix && !is_crate_name_prefix {
            return vec![reference.clone()];
        }

        let module_path = reference.segments.join("::");
        if module_path.is_empty() {
            return vec![reference.clone()];
        }

        // Resolve module path to file
        let file_path = match self.crate_info.resolve_module_path_to_file(&module_path) {
            Ok(path) => path,
            Err(e) => {
                debug!(
                    "Cannot resolve glob for '{}': {e}",
                    reference.to_path_string()
                );
                return vec![reference.clone()];
            }
        };

        // Determine if the target is an inline module within the file.
        // If resolving a shorter prefix yields the same file, the remaining
        // segments are the inline module path.
        let inline_path = self.detect_inline_path(reference, &file_path);
        let inline_refs: Vec<&str> = inline_path.iter().map(String::as_str).collect();

        // Extract public items from the file (optionally descending into inline module)
        let Some(public_items) = extract_public_items(&file_path, &inline_refs) else {
            debug!("Cannot parse '{}' for glob resolution", file_path.display());
            return vec![reference.clone()];
        };

        if public_items.is_empty() {
            return vec![];
        }

        // Build one TypeReference per public item
        public_items
            .into_iter()
            .map(|item| {
                let mut segments = reference.segments.to_vec();
                segments.push(item);
                TypeReference {
                    segments: Segments::from(segments),
                    prefix: reference.prefix,
                    suffix: PathSuffix::None,
                }
            })
            .collect()
    }

    /// Build a mapping from source file to the shortest (file-level) module path.
    ///
    /// When multiple modules share the same source file (inline modules),
    /// the one with the shortest path is the file-level owner.
    fn build_file_root_map(
        modules: &[crate::module::discover::ModuleInfo],
    ) -> HashMap<PathBuf, String> {
        let mut file_root: HashMap<PathBuf, String> = HashMap::new();
        for module in modules {
            file_root
                .entry(module.source().to_path_buf())
                .and_modify(|existing| {
                    if module.path().len() < existing.len() {
                        *existing = module.path().to_string();
                    }
                })
                .or_insert_with(|| module.path().to_string());
        }
        file_root
    }

    /// Compute the inline scope for a module relative to its file root.
    ///
    /// Returns the path segments that identify the inline module within the file.
    /// For example, if `module_path` is `"foo::bar::baz"` and `root_path` is `"foo"`,
    /// returns `["bar", "baz"]`. Returns an empty vec if the module is the file root.
    fn compute_inline_scope(module_path: &str, root_path: &str) -> Vec<String> {
        if module_path == root_path {
            vec![]
        } else {
            module_path
                .strip_prefix(root_path)
                .and_then(|s| s.strip_prefix("::"))
                .map(|s| s.split("::").map(String::from).collect())
                .unwrap_or_default()
        }
    }

    /// Detect which trailing segments of a module path are inline modules
    /// within the resolved file.
    ///
    /// Compares progressive shorter prefixes of the module path against the
    /// resolved file. Once a shorter prefix resolves to a *different* file
    /// (or fails), the remaining segments are the inline module path.
    fn detect_inline_path(&self, reference: &TypeReference, resolved_file: &Path) -> Vec<String> {
        let segments = &reference.segments;

        // Walk from the full path backwards, peeling off one segment at a time.
        // The segments that resolve to the same file are "consumed by" the file;
        // any remainder must be inline modules.
        for split in (1..segments.len()).rev() {
            let prefix_path = segments[..split].join("::");
            match self.crate_info.resolve_module_path_to_file(&prefix_path) {
                Ok(ref parent_file) if parent_file == resolved_file => {
                    // The shorter prefix still resolves to the same file,
                    // so segments[split..] are inline module names.
                    return segments[split..].to_vec();
                }
                _ => {
                    // Different file or resolution failed — this prefix is
                    // a different module, keep peeling.
                }
            }
        }

        // No inline path detected — the file directly corresponds to the module.
        vec![]
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_options_default() {
        let options = AnalysisOptions::default();
        assert!(!options.include_tests);
        assert!(!options.expand_groups);
    }
}
