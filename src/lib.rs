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
//! - **Depth limiting**: Truncate paths to a maximum depth for high-level views

use crate::module::analyzer::{AnalyzerError, CrateAnalyzer};
use crate::module::discover::{CrateInfo, CrateInfoError};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, error, info};

#[allow(dead_code)]
mod analysis;
mod constants;
mod module;
pub mod version;

/// Options for dependency analysis.
///
/// Controls how the analyzer processes modules and formats output.
///
/// # Examples
///
/// ```
/// use crawk::AnalysisOptions;
///
/// // Default options: exclude tests, don't expand groups, no depth limit
/// let options = AnalysisOptions::default();
///
/// // Include test modules and expand grouped imports
/// let options = AnalysisOptions {
///     include_tests: true,
///     expand_groups: true,
///     ..Default::default()
/// };
/// ```
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

    /// Truncate paths at this depth from the crate root.
    ///
    /// For example, with `max_depth = Some(2)`:
    /// - `foo::bar::baz::Thing` becomes `foo::bar`
    ///
    /// `None` means no truncation.
    pub max_depth: Option<usize>,
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
    dependencies: HashMap<String, HashSet<String>>,

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
    pub const fn dependencies(&self) -> &HashMap<String, HashSet<String>> {
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
    pub fn into_sorted_vec(self) -> Vec<String> {
        let all_deps_unique: HashSet<_> = self.dependencies.values().flatten().cloned().collect();
        let mut all_deps_unique: Vec<String> = all_deps_unique.into_iter().collect();
        all_deps_unique.sort();
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
///     max_depth: Some(2),
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

        for module in modules {
            info!(
                "Analyzing module: {} (file: {})",
                module.path(),
                module.source().display()
            );
            match self
                .crate_analyzer
                .parse_file(module.path(), module.source())
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
                if options.expand_groups {
                    debug!(
                        "Expanding groups for reference: {}",
                        reference.to_path_string()
                    );
                    let expanded = reference.expand_suffix();
                    for exp in expanded {
                        debug!("Expanded reference: {}", exp.to_path_string());
                        refs.insert(exp.to_path_string());
                    }
                } else {
                    refs.insert(reference.to_path_string());
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
        assert!(options.max_depth.is_none());
    }
}
