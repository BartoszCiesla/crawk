use crate::cache::ParseCache;
use crate::discover::{CrateInfo, ModuleInfo};
use crate::error::{AnalysisError, Result};
use crate::model::{AnalysisOptions, AnalysisResult};
use crate::parser::CrateAnalyzer;
use crate::reference::{GroupItem, PathSuffix, TypeReference};
use crate::resolve::resolve_glob;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, trace};

/// Expand grouped and aliased imports into individual references.
///
/// - `None` / `Alias` → single reference (alias stripped, segments preserved)
/// - `Glob` → single reference (glob preserved)
/// - `Group` → one reference per group item, recursively expanded for nested groups
pub(crate) fn expand_groups(reference: &TypeReference) -> Vec<TypeReference> {
    match reference.suffix() {
        PathSuffix::None | PathSuffix::Alias(_) => vec![reference.clone_with(true, false)],
        PathSuffix::Glob => vec![reference.clone_with(true, true)],
        PathSuffix::Group(items) => {
            let mut result = Vec::new();
            for item in items {
                match item {
                    GroupItem::Simple(name) | GroupItem::Aliased { name, alias: _ } => {
                        result.push(reference.clone_with(true, false).append_segment(name));
                    }
                    GroupItem::SelfItem { alias: _ } => {
                        result.push(reference.clone_with(true, false));
                    }
                    GroupItem::Glob => {
                        result.push(reference.clone_with(true, true));
                    }
                    GroupItem::Nested {
                        prefix,
                        items: nested_items,
                    } => {
                        let mut nested = reference.clone_with(true, false);
                        for seg in prefix {
                            nested = nested.append_segment(seg);
                        }
                        let nested = nested.with_group(nested_items.clone());
                        result.extend(expand_groups(&nested));
                    }
                }
            }
            result
        }
    }
}

/// Analyzer for Rust module dependencies.
///
/// The main entry point for analyzing module dependencies in a Rust crate.
/// Create an analyzer with a crate root path, then call [`analyze_module`](Self::analyze_module)
/// to analyze specific modules.
///
/// # Thread Safety
///
/// `Analyzer` is **not** `Sync`: [`analyze_module`](Self::analyze_module) requires `&mut self`
/// due to an internal parse cache. To analyze modules in parallel, create a separate
/// `Analyzer` instance per thread.
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
#[derive(Clone)]
pub struct Analyzer {
    /// Crate analyzer
    crate_info: CrateInfo,
    /// Module analyzer
    parser: CrateAnalyzer,
    /// Parse cache: avoids re-reading and re-parsing the same `.rs` file more than once.
    parse_cache: ParseCache,
}

impl Debug for Analyzer {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Analyzer")
            .field("crate_info", &self.crate_info)
            .field("parser", &self.parser)
            .field(
                "parse_cache",
                &format!("<{} entries>", self.parse_cache.len()),
            )
            .finish()
    }
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
    /// Returns [`AnalysisError::ModuleAnalysisFailed`] if there are issues initializing the crate analyzer.
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
        let parser = CrateAnalyzer::new(name);

        Ok(Self {
            crate_info,
            parser,
            parse_cache: ParseCache::new(),
        })
    }

    /// Analyze dependencies for a specific module.
    ///
    /// Recursively analyzes the module and all its submodules, collecting
    /// all internal crate dependencies. Returns an [`AnalysisResult`]
    /// populated according to the given [`AnalysisOptions`].
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
            &mut self.parse_cache,
        )?;

        let source_file = modules
            .first()
            .map(|m| m.source().to_path_buf())
            .unwrap_or_default();

        let file_root = self.build_file_root_map(&modules);
        self.parse_all_modules(modules, &file_root)?;
        let dependencies = self.collect_references(options);

        Ok(AnalysisResult::new(module_path, dependencies, source_file))
    }

    /// Parse each discovered module and accumulate its references into the parser.
    fn parse_all_modules(
        &mut self,
        modules: Vec<ModuleInfo>,
        file_root: &HashMap<PathBuf, String>,
    ) -> Result<()> {
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

            match self.parser.parse_file(
                module.path(),
                module.source(),
                &inline_scope,
                &mut self.parse_cache,
            ) {
                Err(e) => {
                    error!("Error while analyzing module '{}': {e}", module.path());
                    return Err(AnalysisError::ModuleAnalysisFailed {
                        module_path: module.path().to_owned(),
                        file: module.source().to_path_buf(),
                        source: e,
                    });
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
        Ok(())
    }

    /// Transform parsed references: expand groups and resolve globs per the given options.
    fn collect_references(
        &mut self,
        options: &AnalysisOptions,
    ) -> HashMap<String, HashSet<TypeReference>> {
        let mut dependencies = HashMap::new();
        for (module, module_references) in self.parser.all_crate_references() {
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
                    let expanded = expand_groups(reference);
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
                        let resolved =
                            resolve_glob(&r, module, &self.crate_info, &mut self.parse_cache);
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
        dependencies
    }

    /// Build a mapping from source file to the shortest (file-level) module path.
    ///
    /// When multiple modules share the same source file (inline modules),
    /// the one with the shortest path is the file-level owner.
    fn build_file_root_map(&self, modules: &[ModuleInfo]) -> HashMap<PathBuf, String> {
        let mut file_root: HashMap<PathBuf, String> = HashMap::new();
        for module in modules {
            let source_path = module.source().to_path_buf();
            let actual_root = self.find_actual_file_root(module.path(), &source_path);

            match file_root.entry(source_path) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    if actual_root.len() < e.get().len() {
                        *e.get_mut() = actual_root;
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(actual_root);
                }
            }
        }
        file_root
    }

    /// Find the actual file-level module path for a given module.
    ///
    /// This detects if a module is actually an inline module by checking if
    /// shorter prefixes (or empty path for crate root) resolve to the same file.
    fn find_actual_file_root(&self, module_path: &str, source_file: &Path) -> String {
        trace!(
            "Finding file root for module '{}' in file '{}'",
            module_path,
            source_file.display()
        );

        if module_path.is_empty() {
            return String::new();
        }

        // First check if this file is the crate root (lib.rs or main.rs in src/)
        let is_crate_root = source_file.to_string_lossy().ends_with("src/lib.rs")
            || source_file.to_string_lossy().ends_with("src/main.rs");

        if is_crate_root {
            trace!(
                "Source file is crate root, returning empty string for module '{}'",
                module_path
            );
            return String::new();
        }

        let segments: Vec<&str> = module_path.split("::").collect();

        // Try progressively shorter prefixes
        for len in (1..segments.len()).rev() {
            let prefix = segments[..len].join("::");

            trace!("Trying prefix: '{}'", prefix);

            // Check if this prefix resolves to the same file
            match self.crate_info.resolve_module_path_to_file(&prefix) {
                Ok(ref resolved) => {
                    trace!("Prefix '{}' resolved to '{}'", prefix, resolved.display());
                    if resolved == source_file {
                        trace!("Found file root: '{}' for module '{}'", prefix, module_path);
                        return prefix;
                    }
                }
                Err(e) => {
                    trace!("Prefix '{}' failed to resolve: {}", prefix, e);
                }
            }
        }

        // Fallback to the original module path
        trace!("No shorter prefix found, using original: '{}'", module_path);
        module_path.to_owned()
    }

    /// Compute the inline scope for a module relative to its file root.
    ///
    /// Returns the path segments that identify the inline module within the file.
    /// For example, if `module_path` is `"foo::bar::baz"` and `root_path` is `"foo"`,
    /// returns `["bar", "baz"]`. Returns an empty vec if the module is the file root.
    fn compute_inline_scope(module_path: &str, root_path: &str) -> Vec<String> {
        if module_path == root_path {
            vec![]
        } else if root_path.is_empty() {
            // When root_path is empty (crate root), the entire module_path is the inline scope
            module_path.split("::").map(String::from).collect()
        } else {
            module_path
                .strip_prefix(root_path)
                .and_then(|s| s.strip_prefix("::"))
                .map(|s| s.split("::").map(String::from).collect())
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::PathSuffix;

    fn make_ref(segments: &[&str], suffix: PathSuffix) -> TypeReference {
        let base = TypeReference::new(segments.iter().copied());
        match suffix {
            PathSuffix::None => base,
            PathSuffix::Alias(a) => base.with_alias(a),
            PathSuffix::Glob => base.with_glob(),
            PathSuffix::Group(g) => base.with_group(g),
        }
    }

    fn expand_to_segments(r: &TypeReference) -> Vec<Vec<String>> {
        expand_groups(r)
            .into_iter()
            .map(|t| t.segments().to_vec())
            .collect()
    }

    #[test]
    fn test_expand_groups_none_passthrough() {
        let r = make_ref(&["std", "collections"], PathSuffix::None);
        assert_eq!(expand_to_segments(&r), vec![vec!["std", "collections"]]);
    }

    #[test]
    fn test_expand_groups_alias_passthrough() {
        let r = make_ref(
            &["std", "collections", "HashMap"],
            PathSuffix::Alias("Map".into()),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![vec!["std", "collections", "HashMap"]]
        );
    }

    #[test]
    fn test_expand_groups_glob_passthrough() {
        let r = make_ref(&["std", "collections"], PathSuffix::Glob);
        assert_eq!(expand_to_segments(&r), vec![vec!["std", "collections"]]);
    }

    #[test]
    fn test_expand_groups_simple() {
        let r = make_ref(
            &["std", "collections"],
            PathSuffix::Group(vec![
                GroupItem::Simple("HashMap".into()),
                GroupItem::Simple("HashSet".into()),
            ]),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![
                vec!["std", "collections", "HashMap"],
                vec!["std", "collections", "HashSet"],
            ]
        );
    }

    #[test]
    fn test_expand_groups_aliased_uses_original_name() {
        let r = make_ref(
            &["std", "collections"],
            PathSuffix::Group(vec![GroupItem::Aliased {
                name: "HashMap".into(),
                alias: "Map".into(),
            }]),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![vec!["std", "collections", "HashMap"]]
        );
    }

    #[test]
    fn test_expand_groups_self_item_no_alias() {
        let r = make_ref(
            &["std", "collections", "module"],
            PathSuffix::Group(vec![GroupItem::SelfItem { alias: None }]),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![vec!["std", "collections", "module"]]
        );
    }

    #[test]
    fn test_expand_groups_self_item_with_alias() {
        let r = make_ref(
            &["std", "collections", "module"],
            PathSuffix::Group(vec![GroupItem::SelfItem {
                alias: Some("Alias".into()),
            }]),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![vec!["std", "collections", "module"]]
        );
    }

    #[test]
    fn test_expand_groups_self_item_empty_base_no_alias() {
        let r = make_ref(
            &[],
            PathSuffix::Group(vec![GroupItem::SelfItem { alias: None }]),
        );
        assert_eq!(expand_to_segments(&r), vec![vec![] as Vec<String>]);
    }

    #[test]
    fn test_expand_groups_glob_returns_base() {
        let r = make_ref(
            &["std", "collections"],
            PathSuffix::Group(vec![GroupItem::Glob]),
        );
        assert_eq!(expand_to_segments(&r), vec![vec!["std", "collections"]]);
    }

    #[test]
    fn test_expand_groups_nested() {
        let r = make_ref(
            &["std"],
            PathSuffix::Group(vec![GroupItem::Nested {
                prefix: vec!["collections".into()],
                items: vec![
                    GroupItem::Simple("HashMap".into()),
                    GroupItem::Simple("HashSet".into()),
                ],
            }]),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![
                vec!["std", "collections", "HashMap"],
                vec!["std", "collections", "HashSet"],
            ]
        );
    }

    #[test]
    fn test_expand_groups_mixed() {
        let r = make_ref(
            &["m", "n"],
            PathSuffix::Group(vec![
                GroupItem::Simple("a".into()),
                GroupItem::Aliased {
                    name: "b".into(),
                    alias: "B".into(),
                },
                GroupItem::Nested {
                    prefix: vec!["c".into()],
                    items: vec![GroupItem::Simple("x".into()), GroupItem::Simple("y".into())],
                },
                GroupItem::Glob,
            ]),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![
                vec!["m", "n", "a"],
                vec!["m", "n", "b"],
                vec!["m", "n", "c", "x"],
                vec!["m", "n", "c", "y"],
                vec!["m", "n"],
            ]
        );
    }

    #[test]
    fn test_expand_groups_deeply_nested() {
        let r = make_ref(
            &["a"],
            PathSuffix::Group(vec![GroupItem::Nested {
                prefix: vec!["b".into()],
                items: vec![GroupItem::Nested {
                    prefix: vec!["c".into()],
                    items: vec![
                        GroupItem::Simple("d".into()),
                        GroupItem::Simple("e".into()),
                        GroupItem::Simple("f".into()),
                    ],
                }],
            }]),
        );
        assert_eq!(
            expand_to_segments(&r),
            vec![
                vec!["a", "b", "c", "d"],
                vec!["a", "b", "c", "e"],
                vec!["a", "b", "c", "f"],
            ]
        );
    }
}
