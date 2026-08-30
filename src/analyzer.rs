use crate::cache::ParseCache;
use crate::discover::{CrateInfo, ModuleInfo, TargetInfo, TargetKind};
use crate::error::{AnalysisError, Result};
use crate::graph::{self, DependencyGraph, DependencyGraphOptions};
use crate::model::{AnalysisOptions, AnalysisResult};
use crate::parser::CrateAnalyzer;
use crate::reference::{GroupItem, PathPrefix, PathSuffix, TypeReference};
use crate::resolve::resolve_glob;
use crate::rules::{self, CheckOptions, CheckReport, RuleSet};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, trace, warn};

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
    /// Parse cache: avoids re-reading and reparsing the same `.rs` file more than once.
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

    /// List all modules discovered under the given module path.
    ///
    /// Returns a sorted list of all modules found recursively under
    /// `module_path`. If `module_path` is `"lib"`, lists the entire crate.
    ///
    /// # Arguments
    ///
    /// * `module_path` - Root module to list from (e.g., `"lib"`, `"parser"`)
    /// * `include_tests` - Whether to include `#[cfg(test)]` modules
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::ModuleNotFound`] if the module doesn't exist.
    pub fn list_modules(
        &mut self,
        module_path: &str,
        include_tests: bool,
    ) -> Result<Vec<ModuleInfo>> {
        let default_target = self.default_lib_target();
        let modules = match self.crate_info.get_module_tree(
            module_path,
            true,
            include_tests,
            &default_target,
            &mut self.parse_cache,
        ) {
            Ok(mods) => mods,
            Err(ref e) if include_tests && e.is_module_not_found() => {
                self.list_from_test_target(module_path)?
            }
            Err(e) => return Err(e.into()),
        };
        // Rename root entry (empty path) to the requested module path
        let mut modules: Vec<ModuleInfo> = modules
            .into_iter()
            .map(|m| {
                if m.path().is_empty() {
                    m.with_path(module_path.to_owned())
                } else {
                    m
                }
            })
            .collect();
        modules.sort_by(|a, b| a.path().cmp(b.path()));
        modules.dedup_by(|a, b| a.path() == b.path());
        info!("Listed {} modules (after dedup)", modules.len());
        Ok(modules)
    }

    /// Searches integration test targets for the given module path.
    ///
    /// Discovers all test targets, collects their full module trees, and
    /// returns the subtree rooted at `module_path` if found.
    fn list_from_test_target(&mut self, module_path: &str) -> Result<Vec<ModuleInfo>> {
        let targets = self.crate_info.all_targets(true);
        let prefix_with_sep = format!("{module_path}::");

        for (target_info, src_path) in &targets {
            if *target_info.kind() != TargetKind::Test {
                continue;
            }

            let modules = CrateInfo::get_module_tree_for_file(
                src_path,
                target_info,
                true,
                &mut self.parse_cache,
            )?;

            let matched: Vec<ModuleInfo> = modules
                .into_iter()
                .filter(|m| m.path() == module_path || m.path().starts_with(&prefix_with_sep))
                .collect();

            if !matched.is_empty() {
                info!(
                    "Found {} modules for '{}' in test target '{}'",
                    matched.len(),
                    module_path,
                    target_info.name()
                );
                return Ok(matched);
            }
        }

        Err(AnalysisError::ModuleNotFound {
            module_path: module_path.to_owned(),
        })
    }

    /// List modules from all compilation targets in the crate.
    ///
    /// Returns modules from library and binary targets. When `include_tests`
    /// is `true`, also includes integration test targets and `#[cfg(test)]`
    /// modules.
    ///
    /// # Errors
    ///
    /// Returns an error if any target's source file cannot be read or parsed.
    pub fn list_all_modules(&mut self, include_tests: bool) -> Result<Vec<ModuleInfo>> {
        let targets = self.crate_info.all_targets(include_tests);
        let mut all_modules = Vec::new();

        for (target_info, src_path) in &targets {
            let canonical_name = Self::target_module_path(target_info, src_path);
            let modules = match target_info.kind() {
                TargetKind::Lib | TargetKind::Bin => self.crate_info.get_module_tree(
                    &canonical_name,
                    true,
                    include_tests,
                    target_info,
                    &mut self.parse_cache,
                )?,
                TargetKind::Test => CrateInfo::get_module_tree_for_file(
                    src_path,
                    target_info,
                    include_tests,
                    &mut self.parse_cache,
                )?,
            };
            // Rename root entry from "" to canonical name (e.g. "lib", "main")
            let modules = modules.into_iter().map(|m| {
                if m.path().is_empty() {
                    m.with_path(canonical_name.clone())
                } else {
                    m
                }
            });
            all_modules.extend(modules);
        }

        // Sort: target kind (Lib < Bin < Test), then target name, then module path
        all_modules.sort_by(|a, b| {
            a.target()
                .kind()
                .cmp(b.target().kind())
                .then_with(|| a.target().name().cmp(b.target().name()))
                .then_with(|| a.path().cmp(b.path()))
        });
        all_modules.dedup_by(|a, b| a.target() == b.target() && a.path() == b.path());

        info!(
            "Listed {} modules across {} targets",
            all_modules.len(),
            targets.len()
        );
        Ok(all_modules)
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

        let default_target = self.default_lib_target();
        let modules = match self.crate_info.get_module_tree(
            &module_path,
            options.recursive,
            options.include_tests,
            &default_target,
            &mut self.parse_cache,
        ) {
            Ok(mods) => mods,
            Err(ref e) if options.include_tests && e.is_module_not_found() => {
                self.list_from_test_target(&module_path)?
            }
            Err(e) => return Err(e.into()),
        };

        let source_file = modules
            .first()
            .map(|m| m.source().to_path_buf())
            .unwrap_or_default();

        // Use the target from the already-discovered modules so further
        // discovery below hits the same target (lib, bin, or test).
        let target = modules
            .first()
            .map_or_else(|| default_target.clone(), |m| m.target().clone());

        // Build children_map from a recursive view of the module tree so that
        // bare child paths (e.g. `use child::Item`) can be recognised even when
        // the analysis itself is non-recursive.  The parse cache avoids redundant
        // I/O — files discovered here will be cache hits during parse_all_modules.
        let mut children_map = if options.recursive {
            Self::build_children_map(&modules)
        } else {
            let all_modules = match self.crate_info.get_module_tree(
                &module_path,
                true,
                options.include_tests,
                &target,
                &mut self.parse_cache,
            ) {
                Ok(mods) => mods,
                // Fallback: for test targets the module tree is built from
                // the source file directly, not via path resolution. Gated
                // exactly like the discovery call above — a parse or I/O
                // failure must surface as itself, not as "module not found".
                Err(ref e) if options.include_tests && e.is_module_not_found() => {
                    self.list_from_test_target(&module_path)?
                }
                // The queried module already resolved above, so a not-found
                // here is about the recursive walk losing it. Report the
                // fully-qualified query rather than the single segment the
                // discovery error names.
                Err(ref e) if e.is_module_not_found() => {
                    return Err(AnalysisError::ModuleNotFound { module_path });
                }
                Err(e) => return Err(e.into()),
            };
            Self::build_children_map(&all_modules)
        };

        // The discovery above is rooted at `module_path`, so `children_map[""]`
        // is only populated when the query itself targets the crate root — a
        // query scoped to a leaf module (e.g. a `#[cfg(test)] mod tests` with
        // no submodules of its own) sees no crate-root siblings at all. Since
        // bare paths to top-level sibling modules are overwhelmingly reached
        // via `use super::*;` inside test modules, top up `children_map[""]`
        // with a real crate-root listing whenever tests are in scope.
        //
        // `resolved_path` is the normalized form of the query (crate root
        // collapses to ""); `modules.first().path()` already carries this
        // because `get_module_tree` always pushes the queried module itself
        // as the first entry, using the same normalization it applies
        // everywhere else.
        //
        // Restricted to modules that genuinely belong to the lib target's own
        // `mod` tree, verified by membership in a fresh recursive walk of
        // "lib" (the same walk that supplies the root children below — no
        // extra discovery cost). A bin/test target is a *separate* compilation
        // unit from the lib, so its own bare paths can only reach lib modules
        // via an explicit `use <package>::module;` (already handled via
        // `imported_modules`/`resolve_via_import`) — merging in the lib's root
        // children for one of ITS modules would misclassify those as
        // same-target `crate::` refs instead of the correct cross-target
        // `<package>::` form. A file-path/target-tag heuristic can't safely
        // tell these apart (crawk's own module resolution is filesystem-shape
        // based, not `mod`-declaration verified, so e.g. `cli::overview` —
        // owned only by crawk's own bin target's `mod cli;`, absent from
        // lib.rs entirely — still resolves via the generic fallback path);
        // AST membership in the lib's real tree is the only thing that can't
        // lie about this.
        let resolved_path = modules
            .first()
            .map_or(module_path.as_str(), ModuleInfo::path);
        if options.include_tests && !resolved_path.is_empty() {
            let root_modules = self
                .crate_info
                .get_module_tree(
                    "lib",
                    true,
                    options.include_tests,
                    &default_target,
                    &mut self.parse_cache,
                )
                .unwrap_or_else(|e| {
                    warn!(
                        "crate-root module discovery failed during bare-path top-up for '{resolved_path}': {e}"
                    );
                    Vec::new()
                });
            let belongs_to_lib = root_modules.iter().any(|m| m.path() == resolved_path);
            if belongs_to_lib
                && let Some(root_children) = Self::build_children_map(&root_modules).remove("")
            {
                children_map
                    .entry(String::new())
                    .or_default()
                    .extend(root_children);
            }
        }

        let file_root = self.build_file_root_map(&modules);
        // Capture module names BEFORE consuming `modules`. The filter prevents
        // refs parsed for an earlier `analyze_module` call (e.g. lib) from
        // leaking into the result of a later call (e.g. a test target with
        // overlapping or differently-rooted module paths).
        let module_filter: HashSet<String> = modules.iter().map(|m| m.path().to_owned()).collect();
        self.parse_all_modules(modules, &file_root, &children_map)?;
        let dependencies = self.collect_references(options, &children_map, &module_filter);

        Ok(AnalysisResult::new(module_path, dependencies, source_file))
    }

    /// Returns a default `TargetInfo` for the library target.
    fn default_lib_target(&self) -> TargetInfo {
        TargetInfo::new(TargetKind::Lib, self.crate_info.root_package_name())
    }

    /// Computes the module path string to pass to `get_module_tree` for a target.
    ///
    /// Library targets use `"lib"`, binary targets use the file stem of their
    /// source path (e.g., `"main"` for `src/main.rs`).
    fn target_module_path(target: &TargetInfo, src_path: &Path) -> String {
        match target.kind() {
            TargetKind::Lib => "lib".to_owned(),
            TargetKind::Bin => src_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("main")
                .to_owned(),
            TargetKind::Test => src_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("test")
                .to_owned(),
        }
    }

    /// Parse each discovered module and accumulate its references into the parser.
    fn parse_all_modules(
        &mut self,
        modules: Vec<ModuleInfo>,
        file_root: &HashMap<PathBuf, String>,
        children_map: &HashMap<String, HashSet<String>>,
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

            let children = children_map.get(module.path()).cloned().unwrap_or_default();
            // Crate-root top-level modules, passed separately so bare paths to
            // top-level sibling modules (e.g. `inline_modules::inner::greet()`
            // reached via `use super::*;` inside `#[cfg(test)] mod tests`) are
            // recognised as internal refs. Mirrors the edition-2015-style
            // fallback in `is_bare_child`. Kept separate from `children` because
            // the visitor must not apply it to macro invocation paths (macro
            // names live in a different namespace than modules).
            let root_children = children_map.get("").cloned().unwrap_or_default();

            match self.parser.parse_file(
                module.path(),
                module.source(),
                &inline_scope,
                children,
                root_children,
                &mut self.parse_cache,
            ) {
                Err(e) => {
                    error!("Error while analyzing module '{}': {e}", module.path());
                    return Err(AnalysisError::ModuleAnalysisFailed {
                        module_path: module.path().to_owned(),
                        file: module.source().to_path_buf(),
                        message: e.to_string(),
                    });
                }
                Ok(type_list) => {
                    for reference in type_list {
                        debug!("Found reference: {}", reference.to_path_string());
                    }
                }
            }
        }
        Ok(())
    }

    /// Transform parsed references: expand groups and resolve globs per the given options.
    ///
    /// Bare child paths (`use child::Item` without `crate::` prefix) are normalised
    /// to `crate::<parent>::child::Item` so downstream code handles them uniformly.
    ///
    /// `module_filter` scopes iteration to the modules belonging to the current
    /// `analyze_module` call. Required because the underlying `CrateAnalyzer`
    /// accumulates parsed refs across all calls — without this filter, an
    /// `analyze_module("helpers")` call for a test target would re-process refs
    /// parsed for the lib target in an earlier call and emit phantom edges.
    fn collect_references(
        &mut self,
        options: &AnalysisOptions,
        children_map: &HashMap<String, HashSet<String>>,
        module_filter: &HashSet<String>,
    ) -> HashMap<String, HashSet<TypeReference>> {
        let mut dependencies = HashMap::new();
        for (module, module_references) in self
            .parser
            .all_crate_references(children_map, Some(module_filter))
        {
            debug!("Processing module: {}", module);

            // Pre-compute parent segments for bare-path normalisation.
            let module_segments: Vec<String> = if module.is_empty() {
                vec![]
            } else {
                module.split("::").map(String::from).collect()
            };
            let module_children = children_map.get(module.as_str());
            let root_children = children_map.get("");

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

                // Pass 2: normalise bare child paths, then resolve globs if requested
                for r in after_expand {
                    let r = Self::normalise_bare_child(
                        r,
                        &module_segments,
                        module_children,
                        root_children,
                    );

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

    /// Normalise a bare child path to an absolute `crate::` path.
    ///
    /// Two resolution rules (matching `is_bare_child` in the filter):
    /// - **Direct child** (2018+): `use child::Item` in `a::b` → `crate::a::b::child::Item`
    /// - **Root-level** (2015): `use sibling::Item` in `a::b` → `crate::sibling::Item`
    ///
    /// Direct child takes priority when both match (a module has a child
    /// with the same name as a top-level module).
    fn normalise_bare_child(
        r: TypeReference,
        module_segments: &[String],
        module_children: Option<&HashSet<String>>,
        root_children: Option<&HashSet<String>>,
    ) -> TypeReference {
        if r.prefix() != PathPrefix::None {
            return r;
        }

        let Some(first) = r.segments().first() else {
            return r;
        };

        // Direct child match (2018+ rule) — prepend parent module path.
        let is_direct_child = module_children.is_some_and(|ch| ch.contains(first.as_str()));
        if is_direct_child {
            let mut new_segments = module_segments.to_vec();
            new_segments.extend(r.segments().iter().cloned());
            debug!(
                "Normalised bare child: {} → crate::{}",
                r.to_path_string(),
                new_segments.join("::")
            );
            return r.with_segments_and_prefix(new_segments, PathPrefix::Crate);
        }

        // Root-level match (2015 rule) — segments stay as-is, just add crate:: prefix.
        let is_root_sibling = root_children.is_some_and(|ch| ch.contains(first.as_str()));
        if is_root_sibling {
            let segments = r.segments().to_vec();
            debug!(
                "Normalised root-level bare path: {} → crate::{}",
                r.to_path_string(),
                segments.join("::")
            );
            return r.with_segments_and_prefix(segments, PathPrefix::Crate);
        }

        r
    }

    /// Build a mapping from module path to its direct child module names.
    ///
    /// For each module in the tree, extracts the parent path and the child name.
    /// For example, `"cli::overview"` produces parent `"cli"`, child `"overview"`;
    /// top-level module `"cli"` produces parent `""`, child `"cli"`.
    fn build_children_map(modules: &[ModuleInfo]) -> HashMap<String, HashSet<String>> {
        let mut map: HashMap<String, HashSet<String>> = HashMap::new();
        for m in modules {
            let path = m.path();
            if path.is_empty() {
                continue;
            }
            let (parent, child) = match path.rsplit_once("::") {
                Some((p, c)) => (p.to_owned(), c.to_owned()),
                None => (String::new(), path.to_owned()),
            };
            map.entry(parent).or_default().insert(child);
        }
        map
    }

    /// Build a mapping from source file to the shortest (file-level) module path.
    ///
    /// When multiple modules share the same source file (inline modules),
    /// the one with the shortest path is the file-level owner.
    fn build_file_root_map(&mut self, modules: &[ModuleInfo]) -> HashMap<PathBuf, String> {
        let mut file_root: HashMap<PathBuf, String> = HashMap::new();
        for module in modules {
            let source_path = module.source().to_path_buf();
            let (actual_root, _) = self.crate_info.split_inline_scope(
                module.path(),
                &source_path,
                &mut self.parse_cache,
            );
            debug!(
                "File root: '{}' \u{2192} '{}' (file: {})",
                module.path(),
                actual_root,
                source_path.display()
            );

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
        info!(
            "File root map: {} files for {} modules",
            file_root.len(),
            modules.len()
        );
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

    /// Build a complete module-level dependency graph for the crate.
    ///
    /// Discovers all compilation targets, analyses each one, and constructs
    /// a unified set of directed edges between modules. The graph can then
    /// be queried for cycles, orphans, or iterated directly.
    ///
    /// # Arguments
    ///
    /// * `options` — controls which modules are included (`include_tests`),
    ///   path truncation (`depth`), and API annotation (`show_apis`).
    ///
    /// # Errors
    ///
    /// Returns an error if module discovery or analysis fails. Individual
    /// target failures are logged and skipped (matching CLI behaviour).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawk::{Analyzer, DependencyGraphOptions};
    /// use std::path::Path;
    ///
    /// let mut analyzer = Analyzer::new(Path::new("/path/to/crate"))?;
    /// let mut opts = DependencyGraphOptions::default();
    /// opts.depth = Some(1);
    /// let graph = analyzer.dependency_graph(&opts)?;
    ///
    /// for ((source, target), apis) in graph.edges() {
    ///     println!("{source} -> {target}");
    /// }
    /// # Ok::<(), crawk::AnalysisError>(())
    /// ```
    pub fn dependency_graph(
        &mut self,
        options: &DependencyGraphOptions,
    ) -> Result<DependencyGraph> {
        let analysis_options = AnalysisOptions {
            recursive: true,
            include_tests: options.include_tests,
            expand_groups: true,
            resolve_globs: false,
        };

        let all_modules = self.list_all_modules(options.include_tests)?;
        let roots = Self::collect_target_roots(&all_modules);
        info!("Building dependency graph across {} target(s)", roots.len());

        let known_modules: HashSet<String> =
            all_modules.iter().map(|m| m.path().to_owned()).collect();

        let package_name: Option<String> = all_modules
            .iter()
            .find(|m| m.target().kind() == &TargetKind::Lib)
            .map(|m| m.target().name().to_owned());

        let mut all_edges: BTreeMap<graph::Edge, BTreeSet<String>> = BTreeMap::new();
        for root in &roots {
            info!("Analysing target root '{root}'");
            match self.analyze_module(root.as_str(), &analysis_options) {
                Ok(result) => {
                    for (edge, apis) in graph::build_edges(
                        &result,
                        options.depth,
                        &known_modules,
                        package_name.as_deref(),
                        options.show_apis,
                    ) {
                        all_edges.entry(edge).or_default().extend(apis);
                    }
                }
                Err(e) => info!("Skipping target '{root}': {e}"),
            }
        }

        let truncated_modules: BTreeSet<String> = known_modules
            .iter()
            .map(|m| graph::truncate_module_path(m, options.depth).to_owned())
            .collect();

        Ok(DependencyGraph::new(all_edges, truncated_modules))
    }

    /// Check the crate's module dependencies against architectural rules.
    ///
    /// Builds the dependency graph, loads the rule set from `crawk.toml` /
    /// `.crawk.toml` (or [`CheckOptions::config`]), and evaluates the rules.
    /// A [`CheckReport`] with violations is a normal result, **not** an error —
    /// callers map a non-empty report to a non-zero exit code.
    ///
    /// # Arguments
    ///
    /// * `crate_root` — crate root directory, used to discover the config file.
    /// * `opts` — config path and graph options.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph cannot be built, the config file is missing
    /// or malformed, or a rule references an unknown module.
    pub fn check(&mut self, crate_root: &Path, opts: &CheckOptions) -> Result<CheckReport> {
        let graph = self.dependency_graph(&opts.graph_opts())?;
        let path = rules::resolve_config_path(crate_root, opts.config.as_deref())?;
        let rule_set = RuleSet::load(&path, graph.modules())?;
        Ok(rules::evaluate(&rule_set, &graph))
    }

    /// Scaffold a starter config from discovered modules. Returns the path written.
    ///
    /// Builds the dependency graph to enumerate modules, then writes a single
    /// `layers` group skeleton for the user to order. Writes to
    /// [`CheckOptions::config`] when set (`--config`), otherwise `crawk.toml` in
    /// `crate_root`. Refuses to overwrite an existing config.
    ///
    /// # Arguments
    ///
    /// * `crate_root` — crate root directory; the default write location.
    /// * `opts` — graph options plus the optional explicit config path
    ///   (`--config`), which takes precedence over the default location.
    ///
    /// # Errors
    ///
    /// Returns an error if the graph cannot be built, a config already exists,
    /// or the file cannot be written.
    pub fn init_check_config(&mut self, crate_root: &Path, opts: &CheckOptions) -> Result<PathBuf> {
        let graph = self.dependency_graph(&opts.graph_opts())?;
        let crate_name = self.crate_info.root_package_name();
        rules::scaffold_config(
            crate_root,
            opts.config.as_deref(),
            crate_name,
            graph.modules(),
        )
    }

    /// Explain why `source` depends on `target` by listing the concrete references.
    ///
    /// Analyses the `source` module (and its submodules when
    /// [`AnalysisOptions::recursive`] is `true`) and returns only those
    /// [`TypeReference`]s that resolve to the `target` module.
    ///
    /// The result is a map from source submodule path to the set of matching
    /// references. An empty map means `source` has no references to `target`.
    ///
    /// # Arguments
    ///
    /// * `source` - Module path of the dependent (e.g., `"analyzer"`)
    /// * `target` - Module path being depended on (e.g., `"reference"`)
    /// * `options` - Analysis options (recursive, include_tests are respected;
    ///   `expand_groups` is forced to `true` internally for precise matching)
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::ModuleNotFound`] if `source` does not exist.
    /// A non-existent `target` returns an empty map (no error).
    pub fn explain_dependency(
        &mut self,
        source: &str,
        target: &str,
        options: &AnalysisOptions,
    ) -> Result<BTreeMap<String, HashSet<TypeReference>>> {
        let all_modules = self.list_all_modules(options.include_tests)?;
        let known_modules: HashSet<String> =
            all_modules.iter().map(|m| m.path().to_owned()).collect();

        let package_name: Option<String> = all_modules
            .iter()
            .find(|m| m.target().kind() == &TargetKind::Lib)
            .map(|m| m.target().name().to_owned());

        // Accept the synthetic `"lib"` root as a valid target even though it is
        // not a discovered module path: crate-root re-exports resolve to it.
        if target != "lib" && !known_modules.contains(target) {
            info!("Target module '{target}' not found in crate, returning empty result");
            return Ok(BTreeMap::new());
        }

        let analysis_options = AnalysisOptions {
            recursive: options.recursive,
            include_tests: options.include_tests,
            expand_groups: true,
            resolve_globs: false,
        };
        let result = self.analyze_module(source, &analysis_options)?;

        let mut filtered: BTreeMap<String, HashSet<TypeReference>> = BTreeMap::new();
        for (module_key, refs) in result.dependencies() {
            let source_name = if module_key.is_empty() {
                source.to_owned()
            } else {
                module_key.clone()
            };

            for reference in refs {
                if let Some((module_path, _)) = graph::resolve_reference_target(
                    reference,
                    &known_modules,
                    package_name.as_deref(),
                ) && module_path == target
                {
                    filtered
                        .entry(source_name.clone())
                        .or_default()
                        .insert(reference.clone());
                }
            }
        }

        Ok(filtered)
    }

    /// Determine the root module path for each unique compilation target.
    ///
    /// For lib targets the root is always `"lib"`. For binary and test targets
    /// the root is identified from the module's source file path.
    fn collect_target_roots(modules: &[ModuleInfo]) -> Vec<String> {
        let mut groups: HashMap<(TargetKind, String), Vec<&ModuleInfo>> = HashMap::new();
        for m in modules {
            let key = (m.target().kind().clone(), m.target().name().to_owned());
            groups.entry(key).or_default().push(m);
        }

        let mut keys: Vec<_> = groups.keys().cloned().collect();
        keys.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut roots = Vec::new();
        for (kind, name) in &keys {
            let group = &groups[&(kind.clone(), name.clone())];
            let root = match kind {
                TargetKind::Lib => "lib".to_owned(),
                TargetKind::Bin | TargetKind::Test => {
                    Self::find_bin_or_test_root(group).unwrap_or_else(|| name.clone())
                }
            };
            roots.push(root);
        }
        roots
    }

    /// Identify the root module path for a binary or integration-test target.
    ///
    /// Looks for a top-level module whose source file matches known cargo
    /// entry-point patterns (`src/main.rs`, `src/bin/`, `tests/`). Falls back
    /// to the lexicographically smallest top-level module.
    fn find_bin_or_test_root(modules: &[&ModuleInfo]) -> Option<String> {
        let top_level: Vec<_> = modules
            .iter()
            .filter(|m| !m.path().contains("::"))
            .collect();

        let preferred = top_level.iter().find(|m| {
            let src = m.source();
            src.file_name().is_some_and(|n| n == "main.rs")
                || src.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::Normal(n) if n == "bin" || n == "tests"
                    )
                })
        });

        preferred
            .or_else(|| top_level.iter().min_by_key(|m| m.path()))
            .map(|m| m.path().to_owned())
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
        assert_eq!(expand_to_segments(&r), vec![Vec::<String>::new()]);
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
    fn analyzer_debug_format() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let analyzer = Analyzer::new(std::path::Path::new(manifest_dir))
            .expect("crawk's own crate root should be valid");
        let s = format!("{analyzer:?}");
        assert!(s.contains("Analyzer"));
        assert!(s.contains("entries"));
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
