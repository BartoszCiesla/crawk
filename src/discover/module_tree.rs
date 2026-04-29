//! Resolution and tree collection logic for [`CrateInfo`].
//!
//! This module contains the heavy lifting of module discovery: resolving module
//! paths to files on disk, traversing the module tree recursively, and detecting
//! inline modules. All items are additional `impl CrateInfo` blocks — the type
//! itself is defined in the parent module.
//!
//! # Responsibilities
//!
//! - **Path resolution**: mapping `crate::foo::bar` paths to `.rs` files, handling
//!   both `foo.rs` (2018+ style) and `foo/mod.rs` (legacy style) layouts
//! - **Binary target support**: resolving paths rooted at binary targets by matching
//!   source file stems (e.g., `main` → `src/main.rs`)
//! - **Inline module detection**: identifying when a module path resolves to an
//!   inline `mod { ... }` block rather than a separate file
//! - **Tree collection**: recursively or shallowly enumerating submodules, with
//!   optional inclusion of `#[cfg(test)]` modules

use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::cache::ParseCache;

use cargo_metadata::Package;
use syn::Item;

use crate::constants::{LIB_FILE_NAME, MAIN_FILE_NAME, MODULE_FILE_NAME};
use crate::utils::has_cfg_test;
use tracing::{debug, info};

use super::{CrateInfo, CrateInfoError, ModuleInfo, ModuleVisibility, Result, TargetInfo};

impl From<&syn::Visibility> for ModuleVisibility {
    fn from(vis: &syn::Visibility) -> Self {
        match vis {
            syn::Visibility::Public(_) => Self::Public,
            syn::Visibility::Inherited => Self::Inherited,
            syn::Visibility::Restricted(r) => {
                if r.in_token.is_some() {
                    let path = r
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::");
                    Self::InPath(path)
                } else if r.path.is_ident("crate") {
                    Self::Crate
                } else if r.path.is_ident("super") {
                    Self::Super
                } else {
                    // `pub(self)` is semantically private; any other unrecognised
                    // restriction (e.g. a future syn variant) is also treated as
                    // private and logged so it does not go unnoticed.
                    let path = r
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::");
                    if path != "self" {
                        debug!(
                            restriction = %path,
                            "unrecognised pub(…) restriction; treating as private"
                        );
                    }
                    Self::Inherited
                }
            }
        }
    }
}

impl CrateInfo {
    /// Resolves a module path to a file path.
    ///
    /// This method handles both fully qualified paths (with crate name prefix)
    /// and relative module paths.
    ///
    /// # Arguments
    ///
    /// * `module_path` - A module path like `analysis::collect` or `crawk::analysis::collect`
    ///
    /// # Returns
    ///
    /// Returns `Ok(PathBuf)` if the module file is found.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The module path is empty
    /// - The module cannot be found
    /// - The crate root cannot be determined
    pub(super) fn resolve_module(&self, module_path: &str) -> Result<PathBuf> {
        info!("Resolving module: '{module_path}'");
        let package = self.root_package().ok_or(CrateInfoError::PackageNotFound)?;

        self.resolve_module_path_with_crate(package, module_path)
    }

    /// Resolves a module path within a specific package.
    ///
    /// # Arguments
    ///
    /// * `package` - The package to search within
    /// * `module_path` - A module path like `analysis::collect`
    ///
    /// # Errors
    ///
    /// Returns an error if the module path is empty or the module cannot be found.
    fn resolve_module_path(package: &Package, module_path: &str) -> Result<PathBuf> {
        let parts: Vec<&str> = module_path.split("::").collect();

        if parts.is_empty() || module_path.is_empty() {
            return Err(CrateInfoError::EmptyModulePath);
        }

        let crate_root = Self::find_crate_root(package)
            .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))?;

        let crate_root_dir = crate_root
            .parent()
            .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))?;

        let resolved = Self::resolve_module_parts(crate_root_dir, &parts, Some(&crate_root))?
            .ok_or_else(|| CrateInfoError::ModuleNotFound {
                module_path: module_path.to_owned(),
            })?;
        Self::check_within_root(&resolved, crate_root_dir)?;
        Ok(resolved)
    }

    /// Resolves a fully qualified module path that may include the crate name.
    ///
    /// E.g., `crawk::analysis::collect` or just `analysis::collect`
    /// Also treats "lib" as an alias for the library root (same as crate name).
    /// Also matches binary targets by their source file stem (e.g., "main" for main.rs, "app" for app.rs).
    fn resolve_module_path_with_crate(
        &self,
        package: &Package,
        module_path: &str,
    ) -> Result<PathBuf> {
        let parts: Vec<&str> = module_path.split("::").collect();

        if parts.is_empty() || module_path.is_empty() {
            return Err(CrateInfoError::EmptyModulePath);
        }

        // Check if the first part is the crate name itself or "lib" alias
        let is_lib_root = parts[0] == self.root_package_name() || parts[0] == "lib";

        if is_lib_root {
            // Skip the crate name/alias and resolve the rest
            let result = if parts.len() > 1 {
                let remaining_path = parts[1..].join("::");
                Self::resolve_module_path(package, &remaining_path)
            } else {
                // Just the crate name/alias, return the crate root (library)
                Self::find_crate_root(package)
                    .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))
            };
            info!("Resolved '{module_path}' via library root");
            return result;
        }

        // Check if the first part matches any binary target's source file stem
        if Self::find_binary_by_file_stem(package, parts[0]).is_some() {
            let result = if parts.len() > 1 {
                let remaining = parts[1..].join("::");
                Self::resolve_module_path_from_binary(package, parts[0], &remaining)
            } else {
                Self::find_binary_by_file_stem(package, parts[0])
                    .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))
            };
            info!("Resolved '{module_path}' via binary target '{}'", parts[0]);
            return result;
        }

        // Otherwise, resolve as-is
        Self::resolve_module_path(package, module_path)
    }

    /// Finds the crate root file (lib.rs, main.rs, or the first target's src_path).
    ///
    /// Preference order:
    /// 1. Library target (lib.rs)
    /// 2. Binary target (main.rs)
    /// 3. First target's source path
    fn find_crate_root(package: &Package) -> Option<PathBuf> {
        // Prefer lib.rs
        for target in &package.targets {
            if target.is_lib() {
                return Some(target.src_path.as_std_path().to_path_buf());
            }
        }
        // Then main.rs
        for target in &package.targets {
            if target.is_bin() {
                return Some(target.src_path.as_std_path().to_path_buf());
            }
        }
        // Fallback to first target
        package
            .targets
            .first()
            .map(|t| t.src_path.as_std_path().to_path_buf())
    }

    /// Finds a binary target by matching its source file's filename against `"{file_stem}.rs"`.
    pub(super) fn find_binary_by_file_stem(package: &Package, file_stem: &str) -> Option<PathBuf> {
        let expected_filename = format!("{file_stem}.rs");
        package
            .targets
            .iter()
            .find(|t| {
                t.is_bin()
                    && Path::new(t.src_path.as_str())
                        .file_name()
                        .and_then(|f| f.to_str())
                        .is_some_and(|name| name == expected_filename)
            })
            .map(|t| t.src_path.as_std_path().to_path_buf())
    }

    /// Resolves a module path relative to a binary target identified by its source file stem.
    ///
    /// Similar to `resolve_module_path`, but uses the specified binary as the root.
    fn resolve_module_path_from_binary(
        package: &Package,
        file_stem: &str,
        module_path: &str,
    ) -> Result<PathBuf> {
        let parts: Vec<&str> = module_path.split("::").collect();

        if parts.is_empty() || module_path.is_empty() {
            return Err(CrateInfoError::EmptyModulePath);
        }

        let bin_root = Self::find_binary_by_file_stem(package, file_stem)
            .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))?;

        let bin_root_dir = bin_root
            .parent()
            .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))?;

        let resolved = Self::resolve_module_parts(bin_root_dir, &parts, Some(&bin_root))?
            .ok_or_else(|| CrateInfoError::ModuleNotFound {
                module_path: module_path.to_owned(),
            })?;
        Self::check_within_root(&resolved, bin_root_dir)?;
        Ok(resolved)
    }

    /// Validates a single module path segment against path traversal and illegal characters.
    ///
    /// A valid segment is a non-empty Rust identifier with no path separators or
    /// parent-directory references.
    ///
    /// # Errors
    ///
    /// Returns [`CrateInfoError::InvalidModuleSegment`] if the segment is empty, equals `..`,
    /// contains `/` or `\`, or is an absolute path.
    fn validate_segment(part: &str) -> Result<()> {
        if part.is_empty()
            || part == ".."
            || part.contains('/')
            || part.contains('\\')
            || Path::new(part).is_absolute()
        {
            return Err(CrateInfoError::InvalidModuleSegment {
                segment: part.to_owned(),
            });
        }
        Ok(())
    }

    /// Verifies that `path` is contained within `root`.
    ///
    /// Uses [`std::fs::canonicalize`] to resolve symlinks before comparison,
    /// acting as a safety net against traversal that bypasses segment validation.
    ///
    /// # Errors
    ///
    /// Returns [`CrateInfoError::PathTraversal`] if the resolved path escapes `root`,
    /// or [`CrateInfoError::FileRead`] if canonicalization fails.
    fn check_within_root(path: &Path, root: &Path) -> Result<()> {
        let canonical_path = path
            .canonicalize()
            .map_err(|source| CrateInfoError::FileRead {
                path: path.to_path_buf(),
                source,
            })?;
        let canonical_root = root
            .canonicalize()
            .map_err(|source| CrateInfoError::FileRead {
                path: root.to_path_buf(),
                source,
            })?;
        if canonical_path.starts_with(&canonical_root) {
            Ok(())
        } else {
            Err(CrateInfoError::PathTraversal)
        }
    }

    /// Resolves module parts to a file path.
    ///
    /// Checks both `module.rs` (Rust 2018+ style) and `module/mod.rs` (older style) conventions.
    /// If a file-based module is not found, also searches for inline module declarations in the parent file.
    /// When `root_file` is provided, inline modules in that file are checked when the first part
    /// has no corresponding file on disk.
    fn resolve_module_parts(
        base_dir: &Path,
        parts: &[&str],
        root_file: Option<&Path>,
    ) -> Result<Option<PathBuf>> {
        if parts.is_empty() {
            return Ok(None);
        }

        let mut current_dir = base_dir.to_path_buf();
        let mut current_file: Option<PathBuf> = root_file.map(Path::to_path_buf);

        // Navigate through all parts
        for (idx, &part) in parts.iter().enumerate() {
            Self::validate_segment(part)?;
            let is_last = idx == parts.len() - 1;

            let part_dir = current_dir.join(part);

            // Check for `part.rs` (Rust 2018+ style)
            let file_path = current_dir.join(format!("{part}.rs"));
            if file_path.exists() {
                debug!("'{part}' found as {}", file_path.display());
                if is_last {
                    return Ok(Some(file_path));
                }
                // For non-last parts, remember this file and try to navigate into companion directory
                if part_dir.is_dir() {
                    debug!("'{part}' has companion dir, descending");
                    current_file = Some(file_path);
                    current_dir = part_dir;
                    continue;
                }
                // No companion directory, but we have a file - check for inline module
                debug!(
                    "'{part}' no companion dir, checking inline in {}",
                    file_path.display()
                );
                return Self::check_inline_module(&file_path, &parts[idx + 1..]);
            }

            // Check for `part/mod.rs` (older style)
            let mod_path = part_dir.join(MODULE_FILE_NAME);
            if mod_path.exists() {
                debug!("'{part}' found as {}", mod_path.display());
                if is_last {
                    return Ok(Some(mod_path));
                }
                // Navigate into this module's directory
                current_file = Some(mod_path);
                current_dir = part_dir;
                continue;
            }

            // Module not found as a file - check if it's an inline module in the parent file
            if let Some(parent_file) = &current_file {
                debug!(
                    "'{part}' not on disk, checking inline in {}",
                    parent_file.display()
                );
                return Self::check_inline_module(parent_file, &parts[idx..]);
            }

            debug!("'{part}' not found, no parent file to check inline");
            return Ok(None);
        }

        Ok(None)
    }

    /// Checks if a sequence of module names exists as inline modules in the given file.
    /// Returns the file path if all parts exist as nested inline modules, None otherwise.
    fn check_inline_module(file_path: &Path, module_parts: &[&str]) -> Result<Option<PathBuf>> {
        if module_parts.is_empty() {
            return Ok(Some(file_path.to_path_buf()));
        }

        let syntax = Self::parse_source_file(file_path)?;
        Ok(Self::find_nested_inline_module(
            &syntax.items,
            module_parts,
            file_path,
        ))
    }

    /// Recursively checks for nested inline modules within a list of items.
    fn find_nested_inline_module(
        items: &[Item],
        module_parts: &[&str],
        file_path: &Path,
    ) -> Option<PathBuf> {
        if module_parts.is_empty() {
            return Some(file_path.to_path_buf());
        }

        let target_name = module_parts[0];
        for item in items {
            if let Item::Mod(item_mod) = item
                && item_mod.ident == target_name
                && let Some((_, nested_items)) = &item_mod.content
            {
                if module_parts.len() == 1 {
                    return Some(file_path.to_path_buf());
                }
                return Self::find_nested_inline_module(
                    nested_items,
                    &module_parts[1..],
                    file_path,
                );
            }
        }

        None
    }

    /// Collects only the current module (non-recursive, no submodules).
    ///
    /// When `include_tests` is `true`, also includes the direct test module
    /// (marked with `#[cfg(test)]`) if one exists directly under the given module.
    pub(super) fn collect_submodules_shallow(
        file_path: &Path,
        current_module_path: &str,
        current_visibility: ModuleVisibility,
        include_tests: bool,
        target: &TargetInfo,
        cache: &mut ParseCache,
    ) -> Result<Vec<ModuleInfo>> {
        let mut result = Vec::new();

        // Add only the current module itself
        result.push(ModuleInfo::new(
            current_module_path.to_owned(),
            file_path.to_path_buf(),
            current_visibility,
            target.clone(),
        ));

        if include_tests {
            // Read and parse the file to find a direct test module
            let syntax = Self::parse_cached(file_path, cache)?;

            for item in &syntax.items {
                if let Item::Mod(item_mod) = item
                    && has_cfg_test(&item_mod.attrs)
                {
                    let mod_name = item_mod.ident.to_string();
                    let submodule_path = if current_module_path.is_empty() {
                        mod_name
                    } else {
                        format!("{current_module_path}::{mod_name}")
                    };
                    result.push(ModuleInfo::new(
                        submodule_path,
                        file_path.to_path_buf(),
                        ModuleVisibility::from(&item_mod.vis),
                        target.clone(),
                    ));
                }
            }
        }

        Ok(result)
    }

    #[allow(clippy::doc_link_with_quotes)]
    /// Recursively collects all submodules from a file.
    ///
    /// The `inline_scope` parameter specifies which inline modules to descend into
    /// within the file. For file-based modules, this should be empty. For inline
    /// modules, it contains the path segments to navigate (e.g., ["tests"] for
    /// an inline module `mod tests` in the file).
    pub(super) fn collect_submodules_recursive(
        file_path: &Path,
        current_module_path: &str,
        current_visibility: ModuleVisibility,
        inline_scope: &[String],
        include_tests: bool,
        target: &TargetInfo,
        cache: &mut ParseCache,
    ) -> Result<Vec<ModuleInfo>> {
        let base_dir = Self::get_module_base_dir(file_path);
        Self::collect_submodules_with_base_dir(
            file_path,
            &base_dir,
            current_module_path,
            current_visibility,
            inline_scope,
            include_tests,
            target,
            cache,
        )
    }

    /// Gets the base directory for resolving submodules from a file.
    ///
    /// For `module.rs` files, submodules are in a `module/` directory.
    /// For `mod.rs` files, submodules are in the same directory.
    fn get_module_base_dir(file_path: &Path) -> PathBuf {
        let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let parent = file_path.parent().unwrap_or_else(|| Path::new(""));

        if file_name == MODULE_FILE_NAME
            || file_name == LIB_FILE_NAME
            || file_name == MAIN_FILE_NAME
        {
            // Submodules are in the same directory
            parent.to_path_buf()
        } else {
            // For `module.rs`, submodules are in `module/` directory
            let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            parent.join(stem)
        }
    }

    /// Collects the module tree for a crate root file (e.g., `tests/integration.rs`).
    ///
    /// Unlike [`collect_submodules_recursive`](Self::collect_submodules_recursive),
    /// this uses the file's parent directory as base for resolving `mod` declarations.
    /// This is correct for crate roots where `mod helpers;` in `tests/integration.rs`
    /// resolves to `tests/helpers.rs`, not `tests/integration/helpers.rs`.
    pub(super) fn collect_submodules_recursive_crate_root(
        file_path: &Path,
        include_tests: bool,
        target: &TargetInfo,
        cache: &mut ParseCache,
    ) -> Result<Vec<ModuleInfo>> {
        let base_dir = file_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        Self::collect_submodules_with_base_dir(
            file_path,
            &base_dir,
            "",
            ModuleVisibility::Public,
            &[],
            include_tests,
            target,
            cache,
        )
    }

    /// Shared implementation for recursive submodule collection.
    ///
    /// Both [`collect_submodules_recursive`](Self::collect_submodules_recursive) and
    /// [`collect_submodules_recursive_crate_root`](Self::collect_submodules_recursive_crate_root)
    /// delegate here. The only difference is how `base_dir` is computed:
    /// - Regular modules use [`get_module_base_dir`](Self::get_module_base_dir)
    /// - Crate roots use the file's parent directory
    #[allow(clippy::too_many_arguments)]
    fn collect_submodules_with_base_dir(
        file_path: &Path,
        base_dir: &Path,
        current_module_path: &str,
        current_visibility: ModuleVisibility,
        inline_scope: &[String],
        include_tests: bool,
        target: &TargetInfo,
        cache: &mut ParseCache,
    ) -> Result<Vec<ModuleInfo>> {
        let mut result = Vec::new();

        // Add the current module to results
        result.push(ModuleInfo::new(
            current_module_path.to_owned(),
            file_path.to_path_buf(),
            current_visibility,
            target.clone(),
        ));

        // Read and parse the file
        let syntax = Self::parse_cached(file_path, cache)?;

        // Determine which items to iterate over based on inline scope
        let items_to_process = if inline_scope.is_empty() {
            // File-based module - process all top-level items
            &syntax.items
        } else {
            // Inline module - navigate to the inline module and process its items
            Self::get_inline_module_items(&syntax.items, inline_scope)?
        };

        // Extract module declarations
        for item in items_to_process {
            if let Item::Mod(item_mod) = item {
                let mod_name = item_mod.ident.to_string();

                // Check if this is a test module
                let is_test_module = has_cfg_test(&item_mod.attrs);

                // Skip test modules if not including them
                if is_test_module && !include_tests {
                    debug!("Skipping test module: '{mod_name}'");
                    continue;
                }

                // Build the full module path for this submodule
                let submodule_path = if current_module_path.is_empty() {
                    mod_name.clone()
                } else {
                    format!("{current_module_path}::{mod_name}")
                };

                // Check if this is an inline module (has content) or external (file-based)
                let submodule_visibility = ModuleVisibility::from(&item_mod.vis);
                if let Some((_, items)) = &item_mod.content {
                    // Inline module - add it with current file path and recursively process its items
                    info!("Found submodule: '{submodule_path}' (inline)");
                    result.push(ModuleInfo::new(
                        submodule_path.clone(),
                        file_path.to_path_buf(),
                        submodule_visibility,
                        target.clone(),
                    ));
                    result.extend(Self::collect_inline_submodules(
                        items,
                        &submodule_path,
                        file_path,
                        base_dir,
                        include_tests,
                        target,
                        cache,
                    )?);
                } else {
                    // External module - find and parse its file
                    if let Some(sub_mod_file) =
                        Self::resolve_module_parts(base_dir, &[&mod_name], None)?
                    {
                        info!(
                            "Found submodule: '{submodule_path}' \u{2192} {}",
                            sub_mod_file.display()
                        );
                        // External modules are file-based, so inline_scope is empty
                        result.extend(Self::collect_submodules_recursive(
                            &sub_mod_file,
                            &submodule_path,
                            submodule_visibility,
                            &[], // Empty inline scope for file-based modules
                            include_tests,
                            target,
                            cache,
                        )?);
                    } else {
                        debug!("Skipping unresolved external module: '{submodule_path}'");
                    }
                }
            }
        }

        Ok(result)
    }

    /// Collects submodules from inline module items.
    fn collect_inline_submodules(
        items: &[Item],
        current_module_path: &str,
        containing_file: &Path,
        base_dir: &Path,
        include_tests: bool,
        target: &TargetInfo,
        cache: &mut ParseCache,
    ) -> Result<Vec<ModuleInfo>> {
        let mut result = Vec::new();

        for item in items {
            if let Item::Mod(item_mod) = item {
                let mod_name = item_mod.ident.to_string();

                // Check if this is a test module
                let is_test_module = has_cfg_test(&item_mod.attrs);

                // Skip test modules if not including them
                if is_test_module && !include_tests {
                    continue;
                }

                let submodule_path = format!("{current_module_path}::{mod_name}");
                let submodule_visibility = ModuleVisibility::from(&item_mod.vis);

                if let Some((_, nested_items)) = &item_mod.content {
                    // Inline module - record with containing file and recurse into items
                    result.push(ModuleInfo::new(
                        submodule_path.clone(),
                        containing_file.to_path_buf(),
                        submodule_visibility,
                        target.clone(),
                    ));
                    result.extend(Self::collect_inline_submodules(
                        nested_items,
                        &submodule_path,
                        containing_file,
                        base_dir,
                        include_tests,
                        target,
                        cache,
                    )?);
                } else {
                    // File-based module declared inside an inline module
                    if let Some(sub_mod_file) =
                        Self::resolve_module_parts(base_dir, &[&mod_name], None)?
                    {
                        // File-based modules have empty inline scope
                        result.extend(Self::collect_submodules_recursive(
                            &sub_mod_file,
                            &submodule_path,
                            submodule_visibility,
                            &[], // Empty inline scope for file-based modules
                            include_tests,
                            target,
                            cache,
                        )?);
                    } else {
                        debug!(
                            "Skipping unresolved module '{mod_name}' inside inline '{current_module_path}'"
                        );
                    }
                }
            }
        }

        Ok(result)
    }

    /// Computes the visibility of the root module passed to [`CrateInfo::get_module_tree`].
    ///
    /// For the crate root (empty path) this is always [`ModuleVisibility::Public`].
    /// For inline modules the visibility comes from the `mod` declaration inside the file.
    /// For file-based modules the visibility comes from the `mod` declaration in the parent file.
    pub(super) fn compute_root_visibility(
        &self,
        normalized_path: &str,
        file_path: &Path,
        inline_scope: &[String],
        cache: &mut ParseCache,
    ) -> Result<ModuleVisibility> {
        if normalized_path.is_empty() {
            return Ok(ModuleVisibility::Public);
        }

        if !inline_scope.is_empty() {
            // Inline module: navigate the scope within the file to find the `mod` declaration.
            let syntax = Self::parse_cached(file_path, cache)?;
            return Ok(Self::find_visibility_in_scope(&syntax.items, inline_scope));
        }

        // File-based module: locate the `mod name;` in the parent file.
        let segments: Vec<&str> = normalized_path.split("::").collect();
        let module_name = match segments.last() {
            Some(s) => *s,
            None => return Ok(ModuleVisibility::Inherited),
        };

        let parent_file = if segments.len() == 1 {
            // Top-level module — parent is the crate root.
            let package = self.root_package().ok_or(CrateInfoError::PackageNotFound)?;
            Self::find_crate_root(package)
                .ok_or_else(|| CrateInfoError::NoCrateRoot(self.root_package_name().to_owned()))?
        } else {
            let parent_path = segments[..segments.len() - 1].join("::");
            self.resolve_module(&parent_path)?
        };

        let parent_syntax = Self::parse_cached(&parent_file, cache)?;
        Ok(Self::find_mod_visibility_in_items(
            &parent_syntax.items,
            module_name,
        ))
    }

    /// Walks nested inline modules following `scope` and returns the visibility of the last one.
    fn find_visibility_in_scope(items: &[Item], scope: &[String]) -> ModuleVisibility {
        let Some(target) = scope.first() else {
            return ModuleVisibility::Inherited;
        };
        for item in items {
            if let Item::Mod(item_mod) = item
                && item_mod.ident == target.as_str()
            {
                if scope.len() == 1 {
                    return ModuleVisibility::from(&item_mod.vis);
                }
                if let Some((_, nested)) = &item_mod.content {
                    return Self::find_visibility_in_scope(nested, &scope[1..]);
                }
            }
        }
        ModuleVisibility::Inherited
    }

    /// Searches `items` for a `mod name;` or `mod name { }` declaration and returns its visibility.
    fn find_mod_visibility_in_items(items: &[Item], module_name: &str) -> ModuleVisibility {
        for item in items {
            if let Item::Mod(item_mod) = item
                && item_mod.ident == module_name
            {
                return ModuleVisibility::from(&item_mod.vis);
            }
        }
        ModuleVisibility::Inherited
    }

    #[allow(clippy::doc_link_with_quotes)]
    /// Computes the inline scope for a module path within a file.
    ///
    /// Returns the segments of the module path that represent inline modules
    /// within the file. For example, if `module_path` is "foo::bar::tests" and
    /// the file is lib.rs containing an inline module "foo" with an inline module "bar"
    /// with an inline module "tests", returns ["foo", "bar", "tests"].
    ///
    /// Returns empty vector if the module is file-based (not inline).
    pub(super) fn compute_inline_scope_for_path(
        &self,
        module_path: &str,
        file_path: &Path,
    ) -> Vec<String> {
        if module_path.is_empty() {
            return vec![];
        }

        let segments: Vec<&str> = module_path.split("::").collect();

        // Try progressively shorter prefixes to find the file root
        for len in (1..segments.len()).rev() {
            let prefix = segments[..len].join("::");

            if let Ok(resolved) = self.resolve_module_path_to_file(&prefix)
                && resolved == *file_path
            {
                // Found file root at this prefix
                // The remaining segments are inline scope
                return segments[len..].iter().map(ToString::to_string).collect();
            }
        }

        // Check if the file is crate root
        let is_crate_root = file_path.to_string_lossy().ends_with("src/lib.rs")
            || file_path.to_string_lossy().ends_with("src/main.rs");

        if is_crate_root {
            // The entire module path is inline scope within crate root
            return segments.iter().map(ToString::to_string).collect();
        }

        // Not an inline module
        vec![]
    }

    /// Reads and parses a Rust source file (no cache).
    fn parse_source_file(path: &Path) -> Result<syn::File> {
        let content = fs::read_to_string(path).map_err(|e| CrateInfoError::FileRead {
            path: path.to_path_buf(),
            source: e,
        })?;
        syn::parse_file(&content).map_err(|e| CrateInfoError::ParseError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// Reads and parses a Rust source file, returning a cached `Rc<syn::File>`.
    ///
    /// On first access the file is read and parsed; subsequent calls for the same
    /// path return a clone of the existing `Rc` without any I/O or parsing.
    fn parse_cached(path: &Path, cache: &mut ParseCache) -> Result<Rc<syn::File>> {
        cache.get_or_parse(path, Self::parse_source_file)
    }

    #[allow(clippy::doc_link_with_quotes)]
    /// Navigates through nested inline modules and returns the items at the target scope.
    ///
    /// Given a list of items and an inline scope (e.g., ["tests", "submod"]), this function
    /// navigates through the nested inline module declarations and returns the items
    /// inside the target module.
    ///
    /// Returns the top-level items if inline_scope is empty.
    fn get_inline_module_items<'a>(
        items: &'a [Item],
        inline_scope: &[String],
    ) -> Result<&'a [Item]> {
        if inline_scope.is_empty() {
            return Ok(items);
        }

        let module_name = &inline_scope[0];

        for item in items {
            if let Item::Mod(item_mod) = item
                && item_mod.ident == module_name
                && let Some((_, nested_items)) = &item_mod.content
            {
                // Found the inline module, recurse if there are more segments
                if inline_scope.len() > 1 {
                    return Self::get_inline_module_items(nested_items, &inline_scope[1..]);
                }

                return Ok(nested_items);
            }
        }

        // Module not found in inline scope
        Err(CrateInfoError::ModuleNotFound {
            module_path: inline_scope.join("::"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items_from(code: &str) -> Vec<syn::Item> {
        syn::parse_file(code).unwrap().items
    }

    #[test]
    fn base_dir_mod_rs_returns_parent() {
        assert_eq!(
            CrateInfo::get_module_base_dir(Path::new("/src/foo/mod.rs")),
            Path::new("/src/foo")
        );
    }

    #[test]
    fn base_dir_lib_rs_returns_parent() {
        assert_eq!(
            CrateInfo::get_module_base_dir(Path::new("/src/lib.rs")),
            Path::new("/src")
        );
    }

    #[test]
    fn base_dir_main_rs_returns_parent() {
        assert_eq!(
            CrateInfo::get_module_base_dir(Path::new("/src/main.rs")),
            Path::new("/src")
        );
    }

    #[test]
    fn base_dir_named_file_returns_stem_subdir() {
        assert_eq!(
            CrateInfo::get_module_base_dir(Path::new("/src/parser.rs")),
            Path::new("/src/parser")
        );
    }

    #[test]
    fn find_nested_inline_module_empty_parts_returns_file() {
        let items = items_from("pub fn foo() {}");
        let path = Path::new("/fake/file.rs");
        assert_eq!(
            CrateInfo::find_nested_inline_module(&items, &[], path),
            Some(path.to_path_buf())
        );
    }

    #[test]
    fn find_nested_inline_module_finds_existing_module() {
        let items = items_from("pub mod foo { pub fn inner() {} }");
        let path = Path::new("/fake/file.rs");
        assert_eq!(
            CrateInfo::find_nested_inline_module(&items, &["foo"], path),
            Some(path.to_path_buf())
        );
    }

    #[test]
    fn find_nested_inline_module_returns_none_for_missing() {
        let items = items_from("pub fn foo() {}");
        assert!(
            CrateInfo::find_nested_inline_module(&items, &["missing"], Path::new("/f.rs"))
                .is_none()
        );
    }

    #[test]
    fn find_nested_inline_module_finds_nested() {
        let items = items_from("pub mod outer { pub mod inner { pub fn f() {} } }");
        let path = Path::new("/fake/file.rs");
        assert_eq!(
            CrateInfo::find_nested_inline_module(&items, &["outer", "inner"], path),
            Some(path.to_path_buf())
        );
    }

    #[test]
    fn find_nested_inline_module_returns_none_for_missing_nested() {
        let items = items_from("pub mod outer {}");
        assert!(
            CrateInfo::find_nested_inline_module(
                &items,
                &["outer", "nonexistent"],
                Path::new("/f.rs")
            )
            .is_none()
        );
    }

    #[test]
    fn get_inline_module_items_empty_scope_returns_all_items() {
        let items = items_from("pub fn foo() {} pub struct Bar;");
        let result = CrateInfo::get_inline_module_items(&items, &[]).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn get_inline_module_items_finds_named_module() {
        let items = items_from("pub mod foo { pub fn inner() {} pub struct S; }");
        let result = CrateInfo::get_inline_module_items(&items, &["foo".to_owned()]).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn get_inline_module_items_finds_nested_scope() {
        let items = items_from("pub mod a { pub mod b { pub fn deep() {} } }");
        let result =
            CrateInfo::get_inline_module_items(&items, &["a".to_owned(), "b".to_owned()]).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn get_inline_module_items_returns_error_for_missing_module() {
        let items = items_from("pub fn foo() {}");
        let err = CrateInfo::get_inline_module_items(&items, &["nonexistent".to_owned()])
            .err()
            .unwrap();
        assert!(matches!(err, CrateInfoError::ModuleNotFound { .. }));
    }

    #[test]
    fn get_inline_module_items_returns_error_for_missing_nested() {
        let items = items_from("pub mod a { pub fn foo() {} }");
        let err =
            CrateInfo::get_inline_module_items(&items, &["a".to_owned(), "missing".to_owned()])
                .err()
                .unwrap();
        assert!(matches!(err, CrateInfoError::ModuleNotFound { .. }));
    }

    #[test]
    fn test_validate_segment_valid() {
        assert!(CrateInfo::validate_segment("foo").is_ok());
        assert!(CrateInfo::validate_segment("my_module").is_ok());
        assert!(CrateInfo::validate_segment("module123").is_ok());
    }

    #[test]
    fn test_validate_segment_rejects_dotdot() {
        let err = CrateInfo::validate_segment("..").unwrap_err();
        assert!(matches!(err, CrateInfoError::InvalidModuleSegment { .. }));
    }

    #[test]
    fn test_validate_segment_rejects_forward_slash() {
        let err = CrateInfo::validate_segment("foo/bar").unwrap_err();
        assert!(matches!(err, CrateInfoError::InvalidModuleSegment { .. }));
    }

    #[test]
    fn test_validate_segment_rejects_backslash() {
        let err = CrateInfo::validate_segment("foo\\bar").unwrap_err();
        assert!(matches!(err, CrateInfoError::InvalidModuleSegment { .. }));
    }

    #[test]
    fn test_validate_segment_rejects_absolute_path() {
        let err = CrateInfo::validate_segment("/etc/passwd").unwrap_err();
        assert!(matches!(err, CrateInfoError::InvalidModuleSegment { .. }));
    }

    #[test]
    fn test_validate_segment_rejects_empty() {
        let err = CrateInfo::validate_segment("").unwrap_err();
        assert!(matches!(err, CrateInfoError::InvalidModuleSegment { .. }));
    }

    #[test]
    fn test_check_within_root_accepts_child() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("foo.rs");
        fs::write(&child, "").unwrap();
        assert!(CrateInfo::check_within_root(&child, dir.path()).is_ok());
    }

    #[test]
    fn test_check_within_root_rejects_escape() {
        let parent = tempfile::tempdir().unwrap();
        let child = tempfile::tempdir().unwrap();
        // child is outside parent
        let err = CrateInfo::check_within_root(child.path(), parent.path()).unwrap_err();
        assert!(matches!(err, CrateInfoError::PathTraversal));
    }

    #[test]
    fn resolve_module_parts_empty_parts_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = CrateInfo::resolve_module_parts(dir.path(), &[], None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_module_parts_2018_style_foo_rs() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("foo.rs");
        fs::write(&file, "").unwrap();

        let result = CrateInfo::resolve_module_parts(dir.path(), &["foo"], None).unwrap();
        assert_eq!(result, Some(file));
    }

    #[test]
    fn resolve_module_parts_legacy_mod_rs() {
        let dir = tempfile::tempdir().unwrap();
        let mod_dir = dir.path().join("bar");
        fs::create_dir(&mod_dir).unwrap();
        let file = mod_dir.join("mod.rs");
        fs::write(&file, "").unwrap();

        let result = CrateInfo::resolve_module_parts(dir.path(), &["bar"], None).unwrap();
        assert_eq!(result, Some(file));
    }

    #[test]
    fn resolve_module_parts_nested_via_companion_dir() {
        // foo.rs + foo/ companion directory — submodule is in foo/sub.rs
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("foo.rs"), "").unwrap();
        let sub_dir = dir.path().join("foo");
        fs::create_dir(&sub_dir).unwrap();
        let sub_file = sub_dir.join("sub.rs");
        fs::write(&sub_file, "").unwrap();

        let result = CrateInfo::resolve_module_parts(dir.path(), &["foo", "sub"], None).unwrap();
        assert_eq!(result, Some(sub_file));
    }

    #[test]
    fn resolve_module_parts_nested_via_legacy_subdir() {
        // bar/mod.rs + bar/sub.rs
        let dir = tempfile::tempdir().unwrap();
        let bar_dir = dir.path().join("bar");
        fs::create_dir(&bar_dir).unwrap();
        fs::write(bar_dir.join("mod.rs"), "").unwrap();
        let sub_file = bar_dir.join("sub.rs");
        fs::write(&sub_file, "").unwrap();

        let result = CrateInfo::resolve_module_parts(dir.path(), &["bar", "sub"], None).unwrap();
        assert_eq!(result, Some(sub_file));
    }

    #[test]
    fn resolve_module_parts_inline_fallback_when_no_companion_dir() {
        // foo.rs exists (with inline mod inner {}), no foo/ directory
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("foo.rs");
        fs::write(&file, "pub mod inner {}").unwrap();

        let result = CrateInfo::resolve_module_parts(dir.path(), &["foo", "inner"], None).unwrap();
        assert_eq!(result, Some(file));
    }

    #[test]
    fn resolve_module_parts_inline_module_via_root_file() {
        // root_file contains `mod tests {}`, no tests.rs on disk
        let dir = tempfile::tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        fs::write(&lib_rs, "pub mod tests {}").unwrap();

        let result =
            CrateInfo::resolve_module_parts(dir.path(), &["tests"], Some(&lib_rs)).unwrap();
        assert_eq!(result, Some(lib_rs));
    }

    #[test]
    fn resolve_module_parts_returns_none_for_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let result = CrateInfo::resolve_module_parts(dir.path(), &["missing"], None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_module_parts_rejects_invalid_segment() {
        let dir = tempfile::tempdir().unwrap();
        let err = CrateInfo::resolve_module_parts(dir.path(), &[".."], None).unwrap_err();
        assert!(matches!(err, CrateInfoError::InvalidModuleSegment { .. }));
    }

    #[test]
    fn resolve_module_parts_inline_module_not_found_returns_none() {
        // foo.rs exists but has no inline mod missing {}
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("foo.rs");
        fs::write(&file, "pub fn bar() {}").unwrap();

        let result =
            CrateInfo::resolve_module_parts(dir.path(), &["foo", "missing"], None).unwrap();
        assert!(result.is_none());
    }

    // ── ModuleVisibility conversion ──────────────────────────────────────────

    fn parse_vis(s: &str) -> syn::Visibility {
        syn::parse_str(s).unwrap()
    }

    #[test]
    fn vis_pub_maps_to_public() {
        assert_eq!(
            ModuleVisibility::from(&parse_vis("pub")),
            ModuleVisibility::Public
        );
    }

    #[test]
    fn vis_pub_crate_maps_to_crate() {
        assert_eq!(
            ModuleVisibility::from(&parse_vis("pub(crate)")),
            ModuleVisibility::Crate
        );
    }

    #[test]
    fn vis_pub_super_maps_to_super() {
        assert_eq!(
            ModuleVisibility::from(&parse_vis("pub(super)")),
            ModuleVisibility::Super
        );
    }

    #[test]
    fn vis_pub_self_maps_to_inherited() {
        assert_eq!(
            ModuleVisibility::from(&parse_vis("pub(self)")),
            ModuleVisibility::Inherited
        );
    }

    #[test]
    fn vis_inherited_maps_to_inherited() {
        assert_eq!(
            ModuleVisibility::from(&syn::Visibility::Inherited),
            ModuleVisibility::Inherited,
        );
    }

    #[test]
    fn vis_pub_in_path_maps_to_in_path() {
        assert_eq!(
            ModuleVisibility::from(&parse_vis("pub(in crate::foo::bar)")),
            ModuleVisibility::InPath("crate::foo::bar".to_owned()),
        );
    }
}
