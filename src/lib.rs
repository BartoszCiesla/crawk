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
//! let analyzer = Analyzer::new(Path::new("/path/to/crate"));
//! let options = AnalysisOptions::default();
//! let result = analyzer.analyze_module(&["utils", "parser"], &options).unwrap();
//!
//! for dep in result.dependencies() {
//!     println!("{}", dep);
//! }
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

use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::{Path, PathBuf};

mod collector;
mod constants;
mod module;
pub mod version;
mod visitor;

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
    /// The analyzed module path (e.g., `["utils", "parser"]`).
    module_path: Vec<String>,

    /// Set of internal dependencies found, with `crate::` prefix stripped.
    dependencies: HashSet<String>,

    /// Path to the source file that was analyzed.
    source_file: PathBuf,
}

impl AnalysisResult {
    /// Returns the analyzed module path.
    #[must_use]
    pub fn module_path(&self) -> &[String] {
        &self.module_path
    }

    /// Returns the set of dependencies found.
    #[must_use]
    pub const fn dependencies(&self) -> &HashSet<String> {
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
        let mut deps: Vec<_> = self.dependencies.into_iter().collect();
        deps.sort();
        deps
    }
}

/// Error types for analysis operations.
#[derive(Debug)]
pub enum AnalysisError {
    /// The specified module was not found in the crate.
    ModuleNotFound {
        /// The module path that was not found.
        module_path: String,
    },

    /// The crate root directory does not exist or is not a valid Rust project.
    InvalidCrateRoot {
        /// The path that was provided.
        path: PathBuf,
        /// Description of what's wrong.
        reason: String,
    },
}

impl Display for AnalysisError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::ModuleNotFound { module_path } => {
                write!(f, "module not found: {module_path}")
            }
            Self::InvalidCrateRoot { path, reason } => {
                write!(f, "invalid crate root '{}': {}", path.display(), reason)
            }
        }
    }
}

impl Error for AnalysisError {}

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
/// let analyzer = Analyzer::new(Path::new("/path/to/my-crate"));
///
/// // Analyze the "utils" module
/// let result = analyzer.analyze_module(&["utils"], &AnalysisOptions::default())?;
/// println!("Found {} dependencies", result.len());
///
/// // Analyze a nested module with custom options
/// let options = AnalysisOptions {
///     include_tests: true,
///     expand_groups: true,
///     max_depth: Some(2),
/// };
/// let result = analyzer.analyze_module(&["foo", "bar"], &options)?;
/// # Ok::<(), crawk::AnalysisError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Analyzer {
    /// Path to the crate's src directory.
    src_dir: PathBuf,
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
    /// # Examples
    ///
    /// ```no_run
    /// use crawk::Analyzer;
    /// use std::path::Path;
    ///
    /// let analyzer = Analyzer::new(Path::new("/home/user/my-project"));
    /// ```
    #[must_use]
    pub fn new(crate_root: impl AsRef<Path>) -> Self {
        Self {
            src_dir: crate_root.as_ref().join(constants::DEFAULT_SRC_DIR),
        }
    }

    /// Returns the source directory path.
    #[must_use]
    pub fn src_dir(&self) -> &Path {
        &self.src_dir
    }

    /// Find the source file for a module path.
    ///
    /// Returns `Some(path)` if the module exists, `None` otherwise.
    ///
    /// # Arguments
    ///
    /// * `module_path` - Module path components (e.g., `["utils", "parser"]`)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawk::Analyzer;
    /// use std::path::Path;
    ///
    /// let analyzer = Analyzer::new(Path::new("/path/to/crate"));
    /// if let Some(file) = analyzer.find_module(&["utils"]) {
    ///     println!("Found module at: {}", file.display());
    /// }
    /// ```
    #[must_use]
    pub fn find_module(&self, module_path: &[impl AsRef<str>]) -> Option<PathBuf> {
        let path_strings: Vec<String> =
            module_path.iter().map(|s| s.as_ref().to_string()).collect();
        module::locate::find_module_by_path(&self.src_dir, &path_strings)
    }

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
    /// let analyzer = Analyzer::new(Path::new("/path/to/crate"));
    /// let result = analyzer.analyze_module(&["utils"], &AnalysisOptions::default())?;
    ///
    /// for dep in result.dependencies() {
    ///     println!("{}", dep);
    /// }
    /// # Ok::<(), crawk::AnalysisError>(())
    /// ```
    pub fn analyze_module(
        &self,
        module_path: &[impl AsRef<str>],
        options: &AnalysisOptions,
    ) -> Result<AnalysisResult, AnalysisError> {
        let path_strings: Vec<String> =
            module_path.iter().map(|s| s.as_ref().to_string()).collect();
        let module_path_display = path_strings.join("::");

        // Find the module file
        let source_file = module::locate::find_module_by_path(&self.src_dir, &path_strings).ok_or(
            AnalysisError::ModuleNotFound {
                module_path: module_path_display,
            },
        )?;

        // Collect dependencies
        let mut dependencies = HashSet::new();
        collector::collect_use_statements(
            &source_file,
            &mut dependencies,
            options.include_tests,
            &path_strings,
            options.expand_groups,
            options.max_depth,
        );

        Ok(AnalysisResult {
            module_path: path_strings,
            dependencies,
            source_file,
        })
    }

    /// Check if the analyzer's source directory exists.
    ///
    /// Returns `true` if the `src/` directory exists in the crate root.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.src_dir.exists()
    }

    /// Validate that the crate root is a valid Rust project.
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::InvalidCrateRoot`] if the src directory doesn't exist.
    pub fn validate(&self) -> Result<(), AnalysisError> {
        if !self.src_dir.exists() {
            return Err(AnalysisError::InvalidCrateRoot {
                path: self.src_dir.parent().unwrap_or(&self.src_dir).to_path_buf(),
                reason: "src/ directory not found".to_string(),
            });
        }
        Ok(())
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_crate() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        // Create lib.rs
        fs::write(src_dir.join("lib.rs"), "pub mod utils;").unwrap();

        // Create utils.rs with some dependencies
        let mut utils_file = fs::File::create(src_dir.join("utils.rs")).unwrap();
        writeln!(
            utils_file,
            r"
use crate::foo::Bar;
use self::helper::Thing;

fn example() {{
    crate::other::function();
}}
"
        )
        .unwrap();

        temp_dir
    }

    #[test]
    fn test_analyzer_new() {
        let temp_dir = create_test_crate();
        let analyzer = Analyzer::new(temp_dir.path());
        assert!(analyzer.src_dir().ends_with("src"));
    }

    #[test]
    fn test_analyzer_is_valid() {
        let temp_dir = create_test_crate();
        let analyzer = Analyzer::new(temp_dir.path());
        assert!(analyzer.is_valid());

        let invalid_analyzer = Analyzer::new("/nonexistent/path");
        assert!(!invalid_analyzer.is_valid());
    }

    #[test]
    fn test_analyzer_validate() {
        let temp_dir = create_test_crate();
        let analyzer = Analyzer::new(temp_dir.path());
        assert!(analyzer.validate().is_ok());

        let invalid_analyzer = Analyzer::new("/nonexistent/path");
        assert!(invalid_analyzer.validate().is_err());
    }

    #[test]
    fn test_analyzer_find_module() {
        let temp_dir = create_test_crate();
        let analyzer = Analyzer::new(temp_dir.path());

        let result = analyzer.find_module(&["utils"]);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("utils.rs"));

        let result = analyzer.find_module(&["nonexistent"]);
        assert!(result.is_none());
    }

    #[test]
    fn test_analyzer_analyze_module() {
        let temp_dir = create_test_crate();
        let analyzer = Analyzer::new(temp_dir.path());

        let result = analyzer
            .analyze_module(&["utils"], &AnalysisOptions::default())
            .unwrap();

        assert!(!result.is_empty());
        assert!(result.dependencies().contains("foo::Bar"));
        assert!(result.dependencies().contains("other::function"));
    }

    #[test]
    fn test_analyzer_module_not_found() {
        let temp_dir = create_test_crate();
        let analyzer = Analyzer::new(temp_dir.path());

        let result = analyzer.analyze_module(&["nonexistent"], &AnalysisOptions::default());
        assert!(result.is_err());

        if let Err(AnalysisError::ModuleNotFound { module_path }) = result {
            assert_eq!(module_path, "nonexistent");
        } else {
            unreachable!("Expected ModuleNotFound error");
        }
    }

    #[test]
    fn test_analysis_result_into_sorted_vec() {
        let result = AnalysisResult {
            module_path: vec!["test".to_string()],
            dependencies: ["z::Z", "a::A", "m::M"]
                .iter()
                .map(ToString::to_string)
                .collect(),
            source_file: PathBuf::from("/test.rs"),
        };

        let sorted = result.into_sorted_vec();
        assert_eq!(sorted, vec!["a::A", "m::M", "z::Z"]);
    }

    #[test]
    fn test_analysis_options_default() {
        let options = AnalysisOptions::default();
        assert!(!options.include_tests);
        assert!(!options.expand_groups);
        assert!(options.max_depth.is_none());
    }
}
