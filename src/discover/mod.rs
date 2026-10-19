//! Module for locating and resolving Rust module paths within a crate.
//!
//! This module provides the [`CrateInfo`] struct which wraps cargo metadata
//! and provides functionality to resolve module paths (like `analysis::collect`)
//! to their corresponding file paths on disk.

mod module_tree;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::cache::ParseCache;
use tracing::info;

use cargo_metadata::{Metadata, MetadataCommand};
use thiserror::Error;

/// Visibility of a Rust module as declared in source code.
///
/// Mirrors the Rust visibility syntax: `pub`, `pub(crate)`, `pub(super)`,
/// `pub(in path)`, or no keyword (private to the declaring module).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleVisibility {
    /// `pub` — visible to everyone.
    Public,
    /// `pub(crate)` — visible anywhere within the crate.
    Crate,
    /// `pub(super)` — visible to the parent module and its descendants.
    Super,
    /// `pub(in path)` — visible within the subtree rooted at `path`.
    InPath(String),
    /// No visibility modifier — private to the declaring module.
    ///
    /// Note: `pub(self)` is semantically equivalent to no modifier and is also
    /// represented as this variant.
    Inherited,
}

impl fmt::Display for ModuleVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("pub"),
            Self::Crate => f.write_str("pub(crate)"),
            Self::Super => f.write_str("pub(super)"),
            Self::InPath(path) => write!(f, "pub(in {path})"),
            Self::Inherited => Ok(()),
        }
    }
}

/// The kind of compilation target a module belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetKind {
    /// Library target (`lib.rs`).
    Lib,
    /// Binary target (`main.rs` or other `[[bin]]` targets).
    Bin,
    /// Integration test target (`tests/*.rs`).
    Test,
}

impl fmt::Display for TargetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lib => f.write_str("lib"),
            Self::Bin => f.write_str("bin"),
            Self::Test => f.write_str("test"),
        }
    }
}

/// Metadata identifying which compilation target a module belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetInfo {
    /// The kind of target (lib, bin, test).
    kind: TargetKind,
    /// The target name from `Cargo.toml` (e.g., `"crawk"`, `"integration"`).
    name: String,
}

impl TargetInfo {
    /// Creates a new `TargetInfo`.
    #[must_use]
    pub fn new(kind: TargetKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
        }
    }

    /// Returns the target kind.
    #[must_use]
    pub const fn kind(&self) -> &TargetKind {
        &self.kind
    }

    /// Returns the target name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for TargetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TargetKind::Lib => fmt::Display::fmt(&self.kind, f),
            TargetKind::Bin | TargetKind::Test => write!(f, "{}:{}", self.kind, self.name),
        }
    }
}

/// Errors that can occur during crate info operations.
///
/// Reaches library callers wrapped in
/// [`AnalysisError::CrateInfoError`](crate::AnalysisError::CrateInfoError).
///
/// # Examples
///
/// ```no_run
/// use crawk::{AnalysisError, AnalysisOptions, Analyzer, CrateInfoError};
/// # use std::path::Path;
/// # fn main() -> Result<(), AnalysisError> {
/// let mut analyzer = Analyzer::new(Path::new("/path/to/crate"))?;
/// match analyzer.analyze_module("utils::parser", &AnalysisOptions::default()) {
///     Ok(result) => println!("analysed {}", result.module_path()),
///     Err(AnalysisError::CrateInfoError(CrateInfoError::ModuleNotFound { module_path })) => {
///         eprintln!("no module named '{module_path}'");
///     }
///     Err(other) => return Err(other),
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CrateInfoError {
    /// Failed to execute cargo metadata command; see [`MetadataError`] for why.
    #[error("Failed to execute cargo metadata: {0}")]
    MetadataExecution(#[from] MetadataError),

    /// Path points to a workspace root rather than a single crate.
    #[error("workspace support is not yet implemented")]
    WorkspaceRoot,

    /// Package not found in cargo metadata (internal inconsistency).
    #[error("root package not found in cargo metadata")]
    PackageNotFound,

    /// No crate root file (lib.rs or main.rs) found for the package.
    #[error("No crate root file found for package '{0}'")]
    NoCrateRoot(String),

    /// The module path is empty.
    #[error("Module path cannot be empty")]
    EmptyModulePath,

    /// Module not found at the expected path.
    #[error("Module '{module_path}' not found")]
    ModuleNotFound {
        /// The module path that was not found.
        module_path: String,
    },

    /// Failed to read source file.
    #[error("Failed to read file '{path}': {source}")]
    FileRead {
        /// The path of the file that could not be read.
        path: PathBuf,
        /// The underlying IO error.
        source: std::io::Error,
    },

    /// The source file exceeds the maximum allowed size and was not parsed.
    ///
    /// Mirrors the analyzer's own `FileTooLarge` so the discovery path enforces the
    /// same limit; guards against excessive memory use on unexpectedly large files.
    #[error("File too large '{path}': {size} bytes (limit {limit} bytes)")]
    FileTooLarge {
        /// Path to the oversized file.
        path: PathBuf,
        /// Actual file size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        limit: u64,
    },

    /// Failed to parse source file.
    #[error("Failed to parse file '{path}': {message}")]
    ParseError {
        /// The path of the file that could not be parsed.
        path: PathBuf,
        /// The parse error message.
        message: String,
    },

    /// Module path segment is invalid (path traversal or illegal characters).
    #[error("Invalid module path segment '{segment}'")]
    InvalidModuleSegment {
        /// The offending segment.
        segment: String,
    },

    /// Resolved path escapes the crate root directory.
    #[error("Resolved path escapes crate root")]
    PathTraversal,
}

impl CrateInfoError {
    /// Returns `true` if this error is a `ModuleNotFound` variant.
    #[must_use]
    pub const fn is_module_not_found(&self) -> bool {
        matches!(self, Self::ModuleNotFound { .. })
    }
}

/// Why `cargo metadata` could not describe the crate.
///
/// crawk's own summary of `cargo_metadata::Error`, so that crate's error type
/// (and its version) stays out of crawk's public API. Arrives wrapped in
/// [`CrateInfoError::MetadataExecution`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MetadataError {
    /// `cargo metadata` ran, but its exit status reported failure (no
    /// `Cargo.toml`, a broken manifest, ...).
    #[error("`cargo metadata` exited with an error: {stderr}")]
    ExitFailure {
        /// What `cargo metadata` printed on stderr.
        stderr: String,
    },

    /// `cargo metadata` could not be started (no `cargo` on `PATH`, a crate
    /// path that is not a directory, ...).
    #[error("failed to start `cargo metadata`: {source}")]
    Spawn {
        /// The underlying IO error.
        source: std::io::Error,
    },

    /// `cargo metadata` ran, but its output was not valid UTF-8 or JSON.
    #[error("{message}")]
    InvalidOutput {
        /// The original description of what was wrong with the output.
        message: String,
    },
}

impl MetadataError {
    /// Maps the `cargo_metadata` error onto crawk's own cases.
    ///
    /// A plain function rather than `From`: a trait impl naming
    /// `cargo_metadata::Error` would put that type back into the public API.
    /// The match has no catch-all, so a variant added by a `cargo_metadata`
    /// upgrade fails the build here instead of being mapped blindly.
    fn from_cargo(error: cargo_metadata::Error) -> Self {
        use cargo_metadata::Error as Cargo;
        match error {
            Cargo::CargoMetadata { stderr } => Self::ExitFailure { stderr },
            Cargo::Io(source) => Self::Spawn { source },
            unreadable @ (Cargo::Utf8(_) | Cargo::ErrUtf8(_) | Cargo::Json(_) | Cargo::NoJson) => {
                Self::InvalidOutput {
                    message: unreadable.to_string(),
                }
            }
        }
    }
}

/// Result type alias for crate info operations.
pub(crate) type Result<T> = std::result::Result<T, CrateInfoError>;

/// A struct representing information about a Rust module, including its path and source file.
///
/// This struct holds metadata about a specific module within a crate, including
/// the module's fully qualified path and the file system path where it is defined.
/// Modules can be defined either as separate files or as inline modules within
/// another file.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// The full module path (e.g., "analysis::collect")
    module_path: String,

    /// The file path where this module is defined
    source_file: PathBuf,

    /// The visibility of this module as declared in its parent
    visibility: ModuleVisibility,

    /// The compilation target this module belongs to
    target: TargetInfo,
}

impl ModuleInfo {
    /// Creates a new `ModuleInfo` instance.
    ///
    /// # Arguments
    ///
    /// * `module_path` - The fully qualified module path (e.g., "analysis::collect")
    /// * `source_file` - The file system path where this module is defined
    /// * `visibility` - The declared visibility of the module
    /// * `target` - The compilation target this module belongs to
    #[must_use]
    pub fn new(
        module_path: impl Into<String>,
        source_file: PathBuf,
        visibility: ModuleVisibility,
        target: TargetInfo,
    ) -> Self {
        Self {
            module_path: module_path.into(),
            source_file,
            visibility,
            target,
        }
    }

    /// Returns the fully qualified module path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.module_path
    }

    /// Returns the source file path for this module.
    ///
    /// For inline modules (such as test modules defined with `#[cfg(test)]`),
    /// this returns the path of the file containing the inline module definition.
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source_file
    }

    /// Returns the declared visibility of this module.
    #[must_use]
    pub const fn visibility(&self) -> &ModuleVisibility {
        &self.visibility
    }

    /// Returns the compilation target this module belongs to.
    #[must_use]
    pub const fn target(&self) -> &TargetInfo {
        &self.target
    }

    /// Returns a new `ModuleInfo` with the given module path, preserving all other fields.
    #[must_use]
    pub fn with_path(self, new_path: impl Into<String>) -> Self {
        Self {
            module_path: new_path.into(),
            ..self
        }
    }
}

/// Returns `true` if `target` is the package's library, whatever its crate type.
///
/// `lib` is only the default kind: `proc-macro = true` or an explicit
/// `crate-type` makes `cargo metadata` report `proc-macro`, `cdylib`, `rlib`,
/// `dylib` or `staticlib` instead (several at once for `["cdylib", "rlib"]`).
/// A package has at most one library target, so this is the one notion of
/// "the library" — never ask `Target::is_lib()`, which knows `lib` alone.
fn is_library_target(target: &cargo_metadata::Target) -> bool {
    use cargo_metadata::TargetKind as Kind;

    target.kind.iter().any(|kind| {
        matches!(
            kind,
            Kind::Lib | Kind::RLib | Kind::DyLib | Kind::CDyLib | Kind::StaticLib | Kind::ProcMacro
        )
    })
}

/// Memoized filesystem lookups, valid for the lifetime of one [`CrateInfo`].
///
/// Module resolution peels every path prefix of every module (see
/// [`split_inline_scope`](CrateInfo::split_inline_scope)), so the same prefixes
/// are resolved once per descendant without a memo, and every containment check
/// re-canonicalizes the same crate root directory.
#[derive(Debug, Clone, Default)]
struct ResolveCache {
    /// Module path → resolved source file. Successful resolutions only:
    /// `CrateInfoError` is not `Clone`, and replaying a generic error would
    /// lose the original diagnostic.
    modules: HashMap<String, PathBuf>,

    /// Directory → its canonicalized form.
    canonical_dirs: HashMap<PathBuf, PathBuf>,
}

/// A struct that wraps cargo metadata and provides module path resolution.
///
/// This struct holds the metadata for a Rust crate and provides methods to
/// resolve module paths (like `analysis::collect`) to their corresponding
/// file paths on disk.
#[derive(Debug, Clone)]
pub(crate) struct CrateInfo {
    /// The cargo metadata for the crate.
    metadata: Metadata,

    /// The name of the root package, as `Cargo.toml` spells it.
    root_package_name: String,

    /// The name Rust paths use for the root package's library; see
    /// [`lib_crate_name`](Self::lib_crate_name).
    lib_crate_name: String,

    /// Index of the root package within `metadata.packages`, resolved once at
    /// construction. `metadata.packages` is never mutated after `new`, so this
    /// stays valid for the lifetime of the `CrateInfo`.
    root_package_index: usize,

    /// Memoized resolution results. Interior mutability keeps the resolution
    /// API on `&self`; crawk is single-threaded, so `RefCell` suffices.
    resolve_cache: RefCell<ResolveCache>,
}

impl CrateInfo {
    /// Creates a new `CrateInfo` instance from a crate path.
    ///
    /// # Arguments
    ///
    /// * `crate_path` - Path to the root directory of the crate (containing Cargo.toml)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The cargo metadata command fails to execute
    /// - No root package is found in the workspace
    pub(crate) fn new(crate_path: &Path) -> Result<Self> {
        let metadata = MetadataCommand::new()
            .current_dir(crate_path)
            .exec()
            .map_err(MetadataError::from_cargo)?;
        Self::from_metadata(metadata)
    }

    /// Builds a `CrateInfo` from metadata that `cargo metadata` already produced.
    ///
    /// # Errors
    ///
    /// Returns [`CrateInfoError::WorkspaceRoot`] if the metadata has no root package.
    fn from_metadata(metadata: Metadata) -> Result<Self> {
        let root_package = metadata
            .root_package()
            .ok_or(CrateInfoError::WorkspaceRoot)?;
        let root_package_name = root_package.name.to_string();
        let lib_crate_name = Self::lib_crate_name_of(root_package);

        // By id, not by name: `metadata.packages` lists the whole dependency
        // graph, which may hold another package of the same name.
        let root_package_index = metadata
            .packages
            .iter()
            .position(|p| p.id == root_package.id)
            .ok_or(CrateInfoError::WorkspaceRoot)?;

        Ok(Self {
            metadata,
            root_package_name,
            lib_crate_name,
            root_package_index,
            resolve_cache: RefCell::default(),
        })
    }

    /// Returns the memoized file for `module_path`, if it was resolved before.
    pub(super) fn cached_module(&self, module_path: &str) -> Option<PathBuf> {
        self.resolve_cache
            .borrow()
            .modules
            .get(module_path)
            .cloned()
    }

    /// Records a successful module resolution in the memo.
    pub(super) fn cache_module(&self, module_path: &str, resolved: &Path) {
        self.resolve_cache
            .borrow_mut()
            .modules
            .insert(module_path.to_owned(), resolved.to_path_buf());
    }

    /// Canonicalizes `dir`, reusing the result on later calls.
    ///
    /// Containment checks run once per resolved module and always against one
    /// of a handful of target root directories, so the canonical form is worth
    /// keeping instead of re-walking the directory's symlinks every time.
    ///
    /// # Errors
    ///
    /// Returns [`CrateInfoError::FileRead`] if canonicalization fails.
    pub(super) fn canonical_dir(&self, dir: &Path) -> Result<PathBuf> {
        if let Some(cached) = self.resolve_cache.borrow().canonical_dirs.get(dir) {
            return Ok(cached.clone());
        }

        let canonical = dir
            .canonicalize()
            .map_err(|source| CrateInfoError::FileRead {
                path: dir.to_path_buf(),
                source,
            })?;
        self.resolve_cache
            .borrow_mut()
            .canonical_dirs
            .insert(dir.to_path_buf(), canonical.clone());
        Ok(canonical)
    }

    /// Number of memoized module resolutions (test observability).
    #[cfg(test)]
    pub(super) fn cached_module_count(&self) -> usize {
        self.resolve_cache.borrow().modules.len()
    }

    /// Number of directories whose canonical form is memoized (test observability).
    #[cfg(test)]
    pub(super) fn canonical_dir_count(&self) -> usize {
        self.resolve_cache.borrow().canonical_dirs.len()
    }

    /// Returns the name of the root package, as `Cargo.toml` spells it.
    ///
    /// A label for people: it may hold hyphens, so it is never what Rust
    /// source calls the crate. Paths are matched against
    /// [`lib_crate_name`](Self::lib_crate_name) instead.
    #[must_use]
    pub(crate) fn root_package_name(&self) -> &str {
        &self.root_package_name
    }

    /// Returns the name Rust paths use for the root package's library.
    ///
    /// This is what a binary or test target writes to reach the library
    /// (`use my_crate::foo`), and it differs from the package name whenever
    /// that holds a hyphen (`my-crate`) or `[lib] name` overrides it.
    #[must_use]
    pub(crate) fn lib_crate_name(&self) -> &str {
        &self.lib_crate_name
    }

    /// Derives [`lib_crate_name`](Self::lib_crate_name) from cargo metadata.
    ///
    /// The lib target's name already carries a `[lib] name` override; a
    /// package without a library falls back to its own name. Hyphens are
    /// swapped either way: a crate name cannot hold one, the package name is
    /// never normalised, and cargo reports the lib target normalised only
    /// since 1.79.
    fn lib_crate_name_of(package: &cargo_metadata::Package) -> String {
        package
            .targets
            .iter()
            .find(|target| is_library_target(target))
            .map_or(package.name.as_str(), |target| target.name.as_str())
            .replace('-', "_")
    }

    /// Returns `true` if `segment`, leading a module path, names the library
    /// root: the `lib` alias, the crate name Rust source uses, or the package
    /// name as `Cargo.toml` spells it.
    pub(super) fn is_lib_root_alias(&self, segment: &str) -> bool {
        segment == "lib" || segment == self.lib_crate_name || segment == self.root_package_name
    }

    /// Returns all compilation targets in the crate.
    ///
    /// Always includes library and binary targets. When `include_tests` is
    /// `true`, also includes integration test targets.
    pub(crate) fn all_targets(&self, include_tests: bool) -> Vec<(TargetInfo, PathBuf)> {
        let Some(package) = self.root_package() else {
            return vec![];
        };

        let mut result = Vec::new();
        for target in &package.targets {
            let src_path = target.src_path.as_std_path().to_path_buf();
            if is_library_target(target) {
                // Under the crate name, which is what paths in source are
                // matched against; `target.name` may still hold hyphens.
                result.push((
                    TargetInfo::new(TargetKind::Lib, self.lib_crate_name()),
                    src_path,
                ));
            } else if target.is_bin() {
                result.push((
                    TargetInfo::new(TargetKind::Bin, target.name.as_str()),
                    src_path,
                ));
            } else if include_tests && target.kind.contains(&cargo_metadata::TargetKind::Test) {
                result.push((
                    TargetInfo::new(TargetKind::Test, target.name.as_str()),
                    src_path,
                ));
            }
        }
        // Sort: Lib first, then Bin, then Test
        result.sort_by(|a, b| {
            a.0.kind()
                .cmp(b.0.kind())
                .then_with(|| a.0.name().cmp(b.0.name()))
        });
        result
    }

    /// Returns `true` if `file_path` is a compilation target's own entry-point
    /// source file (library root, binary root, or integration-test root), as
    /// reported by `cargo_metadata` — not inferred from a hardcoded filename.
    pub(crate) fn is_target_entry_point(&self, file_path: &Path) -> bool {
        self.root_package().is_some_and(|package| {
            package
                .targets
                .iter()
                .any(|target| target.src_path.as_std_path() == file_path)
        })
    }

    /// Returns the root package from cargo metadata.
    fn root_package(&self) -> Option<&cargo_metadata::Package> {
        self.metadata.packages.get(self.root_package_index)
    }

    /// Returns a list of the given module and all its submodules with their source files.
    ///
    /// This method parses the source files using the `syn` crate to extract
    /// module declarations. It can optionally include test modules (those with
    /// `#[cfg(test)]` attribute).
    ///
    /// # Arguments
    ///
    /// * `module_path` - A module path like `analysis::collect` or `mycrate::analysis`
    /// * `include_tests` - If `true`, includes modules marked with `#[cfg(test)]`
    /// * `recursive` - If `true`, recursively collects all submodules. If `false`, only the
    ///   current module and its direct submodules are returned (without traversing deeper).
    ///   When `include_tests` is `true` and `recursive` is `false`, only the test module
    ///   directly under the given module is included.
    ///
    /// # Returns
    ///
    /// Returns <code>Ok(Vec<[ModuleInfo]>)</code> containing information about each module and its source file.
    /// For inline modules (like `#[cfg(test)] mod tests`), the source file is the containing file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The module path cannot be resolved
    /// - The source file cannot be read
    /// - The source file cannot be parsed
    pub(crate) fn get_module_tree(
        &self,
        module_path: &str,
        recursive: bool,
        include_tests: bool,
        target: &TargetInfo,
        cache: &mut ParseCache,
    ) -> Result<Vec<ModuleInfo>> {
        let file_path = self.resolve_module(module_path, cache)?;

        // Normalize the module path (remove crate name prefix if present)
        let normalized_path = self.normalize_module_path(module_path);
        info!(
            "Module tree: '{}' \u{2192} {}, recursive={recursive}",
            normalized_path,
            file_path.display()
        );

        // Determine if this is an inline module and compute inline scope
        let (_, inline_scope) = self.split_inline_scope(&normalized_path, &file_path, cache);

        let root_visibility =
            self.compute_root_visibility(&normalized_path, &file_path, &inline_scope, cache)?;

        if recursive {
            Self::collect_submodules_recursive(
                &file_path,
                &normalized_path,
                root_visibility,
                &inline_scope,
                include_tests,
                target,
                cache,
            )
        } else {
            Self::collect_submodules_shallow(
                &file_path,
                &normalized_path,
                root_visibility,
                &inline_scope,
                include_tests,
                target,
                cache,
            )
        }
    }

    /// Public wrapper around [`resolve_module`](Self::resolve_module).
    ///
    /// Resolves a module path (e.g., `"foo::bar"`) to the corresponding source file.
    ///
    /// # Errors
    ///
    /// Returns an error if the module cannot be found or the crate root is invalid.
    pub(crate) fn resolve_module_path_to_file(
        &self,
        module_path: &str,
        cache: &mut ParseCache,
    ) -> Result<PathBuf> {
        self.resolve_module(module_path, cache)
    }

    /// Collects the module tree rooted at a source file, without module path resolution.
    ///
    /// Use this for targets (like integration tests) whose root file is not
    /// discoverable through the normal `resolve_module` mechanism.
    ///
    /// # Errors
    ///
    /// Returns an error if the source file cannot be read or parsed.
    pub(crate) fn get_module_tree_for_file(
        src_path: &Path,
        target: &TargetInfo,
        include_cfg_tests: bool,
        cache: &mut ParseCache,
    ) -> Result<Vec<ModuleInfo>> {
        Self::collect_submodules_recursive_crate_root(src_path, include_cfg_tests, target, cache)
    }

    /// Normalizes a module path by removing the crate name prefix if present.
    /// Also normalizes "lib" alias to the crate name.
    /// Also normalizes binary target file stems (e.g., "main" for main.rs).
    /// If the module path is just the crate name/lib/binary, returns empty string.
    fn normalize_module_path(&self, module_path: &str) -> String {
        let parts: Vec<&str> = module_path.split("::").collect();
        // `split` yields at least one segment (`""` for an empty path).
        let first_part = parts[0];

        // Check if first part is the crate name, the package name or "lib" alias
        if self.is_lib_root_alias(first_part) {
            return if parts.len() == 1 {
                // Just the crate name/lib - return empty (analyzing crate root)
                String::new()
            } else {
                parts[1..].join("::")
            };
        }

        // Check if first part is a binary target file stem (e.g., "main", "app")
        if let Some(package) = self.root_package()
            && Self::find_binary_by_file_stem(package, first_part).is_some()
        {
            return if parts.len() == 1 {
                // Just the binary target name - return empty (analyzing binary root)
                String::new()
            } else {
                // Strip the binary target prefix
                parts[1..].join("::")
            };
        }

        module_path.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use test_case::test_case;

    /// Writes a library package named `name` into `dir`: an empty
    /// `src/lib.rs` and a manifest, with `manifest_tail` appended after the
    /// `[package]` table.
    fn write_scratch_package(dir: &Path, name: &str, version: &str, manifest_tail: &str) {
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/lib.rs"), "").unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2024\"\n\n{manifest_tail}"
            ),
        )
        .unwrap();
    }

    /// `cargo metadata` failing on its own terms keeps its stderr, the one part
    /// a caller can act on.
    #[test]
    fn metadata_error_maps_a_failed_run_to_exit_failure() {
        let error = MetadataError::from_cargo(cargo_metadata::Error::CargoMetadata {
            stderr: String::from("error: could not find `Cargo.toml`"),
        });

        assert!(
            matches!(&error, MetadataError::ExitFailure { stderr } if stderr == "error: could not find `Cargo.toml`"),
            "{error:?}"
        );
    }

    /// A failed spawn keeps the IO error as its source, so the `Caused by:`
    /// chain and `kind()` survive the mapping.
    #[test]
    fn metadata_error_maps_a_failed_spawn_to_spawn_keeping_the_source() {
        let error = MetadataError::from_cargo(cargo_metadata::Error::Io(std::io::Error::from(
            std::io::ErrorKind::NotFound,
        )));

        assert!(
            matches!(&error, MetadataError::Spawn { source } if source.kind() == std::io::ErrorKind::NotFound),
            "{error:?}"
        );
        assert!(std::error::Error::source(&error).is_some());
    }

    /// Every unreadable-output variant collapses into one case that keeps the
    /// original message word for word.
    #[test_case(cargo_metadata::Error::NoJson ; "no json")]
    #[test_case(
        cargo_metadata::Error::Utf8(String::from_utf8(vec![0xff]).unwrap_err().utf8_error())
        ; "stdout not utf8"
    )]
    #[test_case(
        cargo_metadata::Error::ErrUtf8(String::from_utf8(vec![0xff]).unwrap_err())
        ; "stderr not utf8"
    )]
    fn metadata_error_maps_unreadable_output_to_invalid_output(cargo_error: cargo_metadata::Error) {
        let expected = cargo_error.to_string();

        let error = MetadataError::from_cargo(cargo_error);

        assert!(
            matches!(&error, MetadataError::InvalidOutput { .. }),
            "{error:?}"
        );
        assert_eq!(error.to_string(), expected);
    }

    /// A scratch package named `name` whose library declares one module, `foo`.
    fn scratch_package_with_foo(name: &str, manifest_tail: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write_scratch_package(dir.path(), name, "0.1.0", manifest_tail);
        fs::write(dir.path().join("src/lib.rs"), "pub mod foo;\n").unwrap();
        fs::write(dir.path().join("src/foo.rs"), "").unwrap();
        dir
    }

    /// The cached index must select the real root package that
    /// `Metadata::root_package` identifies, not a namesake — a wrong or stale
    /// index would silently return a different package.
    #[test]
    fn root_package_uses_cached_index_pointing_at_the_root_package() {
        let info = CrateInfo::new(Path::new("fixtures/modules")).unwrap();
        let cached = info.root_package().expect("cached root package");
        let expected = info.metadata.root_package().expect("metadata root package");
        assert_eq!(cached.id, expected.id);
        assert_eq!(cached.name.to_string(), info.root_package_name());
    }

    /// A dependency graph can hold another package with the root's name — a
    /// crate whose dev-dependencies pull in a released version of itself, as
    /// `syn`, `quote` and `proc-macro2` do — and `metadata.packages` lists the
    /// whole graph, so the root is told apart by its id, never by its name.
    #[test]
    fn root_package_is_not_a_namesake_from_the_dependency_graph() {
        let dir = tempfile::tempdir().unwrap();
        write_scratch_package(
            dir.path(),
            "scratch",
            "0.2.0",
            "[dependencies]\nolder = { package = \"scratch\", path = \"older\" }\n",
        );
        write_scratch_package(&dir.path().join("older"), "scratch", "0.1.0", "");

        let info = CrateInfo::new(dir.path()).unwrap();

        // `cargo metadata` sorts packages by name, then version: the fixture
        // only bites while the namesake is listed ahead of the root.
        assert_eq!(info.metadata.packages[0].version.to_string(), "0.1.0");
        let root = info.root_package().expect("cached root package");
        assert_eq!(root.version.to_string(), "0.2.0");
    }

    /// Rust source never sees the package name: it calls the library by the
    /// name of its lib target, where the hyphens of `my-crate` are underscores.
    #[test]
    fn lib_crate_name_swaps_the_hyphens_of_the_package_name() {
        let dir = scratch_package_with_foo("my-crate", "");

        let info = CrateInfo::new(dir.path()).unwrap();

        assert_eq!(info.lib_crate_name(), "my_crate");
        assert_eq!(info.root_package_name(), "my-crate");
    }

    /// `[lib] name` renames the library and leaves the package name alone.
    #[test]
    fn lib_crate_name_follows_a_lib_name_override() {
        let dir = scratch_package_with_foo("my-crate", "[lib]\nname = \"renamed_lib\"\n");

        let info = CrateInfo::new(dir.path()).unwrap();

        assert_eq!(info.lib_crate_name(), "renamed_lib");
        assert_eq!(info.root_package_name(), "my-crate");
    }

    /// A package without a library has no lib target to ask, so its own name
    /// stands in — spelled the way rustc names the crate of its binary.
    #[test]
    fn lib_crate_name_of_a_bin_only_package_is_its_normalised_name() {
        let dir = tempfile::tempdir().unwrap();
        write_scratch_package(dir.path(), "my-tool", "0.1.0", "");
        fs::rename(
            dir.path().join("src/lib.rs"),
            dir.path().join("src/main.rs"),
        )
        .unwrap();

        let info = CrateInfo::new(dir.path()).unwrap();

        assert_eq!(info.lib_crate_name(), "my_tool");
    }

    /// Cargo before 1.79 reported the lib target under the package's own,
    /// hyphenated name, and `cargo metadata` runs on whatever toolchain the
    /// analysed project pins — so the hyphens are swapped here, not trusted
    /// to be gone.
    #[test]
    fn lib_crate_name_normalises_a_lib_target_reported_with_hyphens() {
        let dir = scratch_package_with_foo("my-crate", "");
        let mut metadata = MetadataCommand::new()
            .current_dir(dir.path())
            .exec()
            .unwrap();
        let lib = metadata
            .packages
            .iter_mut()
            .flat_map(|package| &mut package.targets)
            .find(|target| is_library_target(target))
            .expect("lib target");
        lib.name = String::from("my-crate");

        let info = CrateInfo::from_metadata(metadata).unwrap();

        assert_eq!(info.lib_crate_name(), "my_crate");
        let targets = info.all_targets(false);
        assert_eq!(targets[0].0, TargetInfo::new(TargetKind::Lib, "my_crate"));
    }

    /// A module path may lead with either spelling of a hyphenated package:
    /// the crate name Rust source uses, or the package name of `Cargo.toml`.
    #[test]
    fn module_path_reaches_the_library_by_crate_name_and_by_package_name() {
        let dir = scratch_package_with_foo("my-crate", "");
        let info = CrateInfo::new(dir.path()).unwrap();
        let mut cache = ParseCache::new();

        for root in ["my_crate", "my-crate", "lib"] {
            let nested = format!("{root}::foo");

            assert_eq!(info.normalize_module_path(root), "", "{root}");
            assert_eq!(info.normalize_module_path(&nested), "foo", "{nested}");
            let lib_file = info.resolve_module_path_to_file(root, &mut cache).unwrap();
            assert!(lib_file.ends_with("src/lib.rs"), "{root}: {lib_file:?}");
            let foo_file = info
                .resolve_module_path_to_file(&nested, &mut cache)
                .unwrap();
            assert!(foo_file.ends_with("src/foo.rs"), "{nested}: {foo_file:?}");
        }
    }

    /// Under `[lib] name` the default crate name `my_crate` names nothing:
    /// only the override and the package name lead to the library.
    #[test]
    fn module_path_does_not_reach_a_renamed_library_by_its_default_crate_name() {
        let dir = scratch_package_with_foo("my-crate", "[lib]\nname = \"renamed_lib\"\n");
        let info = CrateInfo::new(dir.path()).unwrap();
        let mut cache = ParseCache::new();

        assert_eq!(info.normalize_module_path("renamed_lib::foo"), "foo");
        assert_eq!(info.normalize_module_path("my-crate::foo"), "foo");
        assert_eq!(info.normalize_module_path("my_crate::foo"), "my_crate::foo");
        assert!(
            info.resolve_module_path_to_file("my_crate::foo", &mut cache)
                .is_err()
        );
    }

    /// `[lib]` sections that leave cargo's plain `lib` kind out of the
    /// target's `kind`: `cargo metadata` reports `proc-macro`, `cdylib`, ...
    /// for them, yet each is still the package's one library.
    const NON_LIB_KIND_MANIFESTS: [&str; 6] = [
        "[lib]\nproc-macro = true\n",
        "[lib]\ncrate-type = [\"cdylib\"]\n",
        "[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n",
        "[lib]\ncrate-type = [\"rlib\"]\n",
        "[lib]\ncrate-type = [\"dylib\"]\n",
        "[lib]\ncrate-type = [\"staticlib\"]\n",
    ];

    /// A library whose kind is not `lib` is still the library target:
    /// without it `list`, `deps` and `check` see no module at all.
    #[test]
    fn all_targets_lists_a_library_of_any_crate_type() {
        for manifest_tail in NON_LIB_KIND_MANIFESTS {
            let dir = scratch_package_with_foo("my-macros", manifest_tail);
            let info = CrateInfo::new(dir.path()).unwrap();

            let targets = info.all_targets(false);

            assert_eq!(targets.len(), 1, "{manifest_tail:?}");
            assert_eq!(
                targets[0].0,
                TargetInfo::new(TargetKind::Lib, "my_macros"),
                "{manifest_tail:?}"
            );
            assert!(
                targets[0].1.ends_with("src/lib.rs"),
                "{manifest_tail:?}: {:?}",
                targets[0].1
            );
        }
    }

    /// The crate name comes from the library target, so a `[lib] name`
    /// override must be found on a library of any crate type, too — not
    /// replaced by the package name.
    #[test]
    fn lib_crate_name_follows_a_lib_name_override_on_a_library_of_any_crate_type() {
        for manifest_tail in NON_LIB_KIND_MANIFESTS {
            let tail = manifest_tail.replacen("[lib]\n", "[lib]\nname = \"renamed_lib\"\n", 1);
            let dir = scratch_package_with_foo("my-macros", &tail);

            let info = CrateInfo::new(dir.path()).unwrap();

            assert_eq!(info.lib_crate_name(), "renamed_lib", "{tail:?}");
            assert_eq!(info.root_package_name(), "my-macros", "{tail:?}");
            assert_eq!(
                info.all_targets(false)[0].0,
                TargetInfo::new(TargetKind::Lib, "renamed_lib"),
                "{tail:?}"
            );
        }
    }

    /// The bare crate name names the library root even when a binary sits
    /// beside a library that is not of kind `lib`: the binary is only the
    /// fallback when no library exists.
    #[test]
    fn crate_root_is_the_library_of_any_crate_type_not_the_binary() {
        for manifest_tail in NON_LIB_KIND_MANIFESTS {
            let dir = scratch_package_with_foo("my-macros", manifest_tail);
            fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
            let info = CrateInfo::new(dir.path()).unwrap();
            let mut cache = ParseCache::new();

            let root = info
                .resolve_module_path_to_file("my_macros", &mut cache)
                .unwrap();

            assert!(root.ends_with("src/lib.rs"), "{manifest_tail:?}: {root:?}");
        }
    }

    /// Integration tests reach a non-`lib` library through its crate name, so
    /// the module list has to carry the library under that name.
    #[test]
    fn all_targets_with_tests_keeps_the_library_of_any_crate_type_first() {
        for manifest_tail in NON_LIB_KIND_MANIFESTS {
            let dir = scratch_package_with_foo("my-macros", manifest_tail);
            fs::create_dir_all(dir.path().join("tests")).unwrap();
            fs::write(dir.path().join("tests/it.rs"), "").unwrap();
            let info = CrateInfo::new(dir.path()).unwrap();

            let kinds: Vec<_> = info
                .all_targets(true)
                .into_iter()
                .map(|(target, _)| target)
                .collect();

            assert_eq!(
                kinds,
                [
                    TargetInfo::new(TargetKind::Lib, "my_macros"),
                    TargetInfo::new(TargetKind::Test, "it"),
                ],
                "{manifest_tail:?}"
            );
        }
    }
}
