use crate::reference::TypeReference;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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
/// and all its submodules. Created by [`Analyzer::analyze_module`](crate::Analyzer::analyze_module)
/// using the options specified in [`AnalysisOptions`].
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
    /// Creates a new analysis result.
    #[must_use]
    pub(crate) const fn new(
        module_path: String,
        dependencies: HashMap<String, HashSet<TypeReference>>,
        source_file: PathBuf,
    ) -> Self {
        Self {
            module_path,
            dependencies,
            source_file,
        }
    }

    /// Returns the analyzed module path.
    #[must_use]
    pub fn module_path(&self) -> &str {
        &self.module_path
    }

    /// Returns the internal crate references found, grouped by module path.
    ///
    /// The map key is the **module path** (e.g., `"utils::parser"`). The value is the set of
    /// [`TypeReference`] items found in that module's source.
    ///
    /// With [`AnalysisOptions::recursive`] set to `false` (default), the map contains exactly
    /// one entry — for the module passed to [`Analyzer::analyze_module`](crate::Analyzer::analyze_module).
    /// With `recursive: true`, the map contains one entry per discovered submodule (e.g.,
    /// `"utils"`, `"utils::parser"`, `"utils::lexer"`, …).
    ///
    /// To get a flat, deduplicated list across all modules, use
    /// [`into_sorted_vec`](Self::into_sorted_vec) instead.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_options_default() {
        let options = AnalysisOptions::default();
        assert!(!options.include_tests);
        assert!(!options.expand_groups);
    }

    #[test]
    fn test_analysis_result_source_file() {
        let result = AnalysisResult::new(
            "foo::bar".to_owned(),
            HashMap::new(),
            PathBuf::from("/tmp/test.rs"),
        );
        assert_eq!(result.source_file(), Path::new("/tmp/test.rs"));
    }

    #[test]
    fn test_analysis_result_len_and_is_empty() {
        let empty_result = AnalysisResult::new("empty".to_owned(), HashMap::new(), PathBuf::new());
        assert_eq!(empty_result.len(), 0);
        assert!(empty_result.is_empty());

        let mut deps = HashMap::new();
        deps.insert("mod_a".to_owned(), HashSet::new());
        deps.insert("mod_b".to_owned(), HashSet::new());
        let non_empty_result = AnalysisResult::new("root".to_owned(), deps, PathBuf::new());
        assert_eq!(non_empty_result.len(), 2);
        assert!(!non_empty_result.is_empty());
    }

    #[test]
    fn test_analysis_result_module_path() {
        let result =
            AnalysisResult::new("foo::bar::baz".to_owned(), HashMap::new(), PathBuf::new());
        assert_eq!(result.module_path(), "foo::bar::baz");
    }
}
