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

use cargo_metadata::Package;
use syn::{Attribute, Item, Meta};

use crate::constants::{
    ATTR_CFG, LIB_FILE_NAME, MAIN_FILE_NAME, MODULE_FILE_NAME, MODULE_NAME_TEST,
};

use super::{CrateInfo, CrateInfoError, ModuleInfo, Result};

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
        let package = self
            .metadata
            .packages
            .iter()
            .find(|p| p.name == self.root_package_name)
            .ok_or(CrateInfoError::PackageNotFound)?;

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

        Self::resolve_module_parts(crate_root_dir, &parts, Some(&crate_root))?.ok_or_else(|| {
            CrateInfoError::ModuleNotFound {
                module_path: module_path.to_string(),
            }
        })
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
        let is_lib_root = parts[0] == self.root_package_name || parts[0] == "lib";

        if is_lib_root {
            // Skip the crate name/alias and resolve the rest
            return if parts.len() > 1 {
                let remaining_path = parts[1..].join("::");
                Self::resolve_module_path(package, &remaining_path)
            } else {
                // Just the crate name/alias, return the crate root (library)
                Self::find_crate_root(package)
                    .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))
            };
        }

        // Check if the first part matches any binary target's source file stem
        if Self::find_binary_by_file_stem(package, parts[0]).is_some() {
            return if parts.len() > 1 {
                let remaining = parts[1..].join("::");
                Self::resolve_module_path_from_binary(package, parts[0], &remaining)
            } else {
                Self::find_binary_by_file_stem(package, parts[0])
                    .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))
            };
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
                return Some(target.src_path.clone().into());
            }
        }
        // Then main.rs
        for target in &package.targets {
            if target.is_bin() {
                return Some(target.src_path.clone().into());
            }
        }
        // Fallback to first target
        package.targets.first().map(|t| t.src_path.clone().into())
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
            .map(|t| t.src_path.clone().into())
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

        Self::resolve_module_parts(bin_root_dir, &parts, Some(&bin_root))?.ok_or_else(|| {
            CrateInfoError::ModuleNotFound {
                module_path: module_path.to_string(),
            }
        })
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
            let is_last = idx == parts.len() - 1;

            let part_dir = current_dir.join(part);

            // Check for `part.rs` (Rust 2018+ style)
            let file_path = current_dir.join(format!("{part}.rs"));
            if file_path.exists() {
                if is_last {
                    return Ok(Some(file_path));
                }
                // For non-last parts, remember this file and try to navigate into companion directory
                if part_dir.is_dir() {
                    current_file = Some(file_path);
                    current_dir = part_dir;
                    continue;
                }
                // No companion directory, but we have a file - check for inline module
                return Self::check_inline_module(&file_path, &parts[idx + 1..]);
            }

            // Check for `part/mod.rs` (older style)
            let mod_path = part_dir.join(MODULE_FILE_NAME);
            if mod_path.exists() {
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
                return Self::check_inline_module(parent_file, &parts[idx..]);
            }

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
        include_tests: bool,
    ) -> Result<Vec<ModuleInfo>> {
        let mut result = Vec::new();

        // Add only the current module itself
        result.push(ModuleInfo::new(
            current_module_path.to_string(),
            file_path.to_path_buf(),
        ));

        if include_tests {
            // Read and parse the file to find a direct test module
            let syntax = Self::parse_source_file(file_path)?;

            for item in &syntax.items {
                if let Item::Mod(item_mod) = item
                    && Self::has_cfg_test(&item_mod.attrs)
                {
                    let mod_name = item_mod.ident.to_string();
                    let submodule_path = if current_module_path.is_empty() {
                        mod_name
                    } else {
                        format!("{current_module_path}::{mod_name}")
                    };
                    result.push(ModuleInfo::new(submodule_path, file_path.to_path_buf()));
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
        inline_scope: &[String],
        include_tests: bool,
    ) -> Result<Vec<ModuleInfo>> {
        let mut result = Vec::new();

        // Add the current module to results
        result.push(ModuleInfo::new(
            current_module_path.to_string(),
            file_path.to_path_buf(),
        ));

        // Read and parse the file
        let syntax = Self::parse_source_file(file_path)?;

        // Get the directory containing this file for resolving submodules
        let base_dir = Self::get_module_base_dir(file_path);

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
                let is_test_module = Self::has_cfg_test(&item_mod.attrs);

                // Skip test modules if not including them
                if is_test_module && !include_tests {
                    continue;
                }

                // Build the full module path for this submodule
                let submodule_path = if current_module_path.is_empty() {
                    mod_name.clone()
                } else {
                    format!("{current_module_path}::{mod_name}")
                };

                // Check if this is an inline module (has content) or external (file-based)
                if let Some((_, items)) = &item_mod.content {
                    // Inline module - add it with current file path and recursively process its items
                    result.push(ModuleInfo::new(
                        submodule_path.clone(),
                        file_path.to_path_buf(),
                    ));
                    result.extend(Self::collect_inline_submodules(
                        items,
                        &submodule_path,
                        file_path,
                        &base_dir,
                        include_tests,
                    )?);
                } else {
                    // External module - find and parse its file
                    if let Some(sub_mod_file) =
                        Self::resolve_module_parts(&base_dir, &[&mod_name], None)?
                    {
                        // External modules are file-based, so inline_scope is empty
                        result.extend(Self::collect_submodules_recursive(
                            &sub_mod_file,
                            &submodule_path,
                            &[], // Empty inline scope for file-based modules
                            include_tests,
                        )?);
                    }
                }
            }
        }

        Ok(result)
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

    /// Checks if an item has a `#[cfg(test)]` attribute.
    ///
    /// Matches `#[cfg(test)]`, `#[cfg(all(test, ...))]`, `#[cfg(any(test, ...))]`,
    /// but not `#[cfg(not(test))]`.
    fn has_cfg_test(attrs: &[Attribute]) -> bool {
        use proc_macro2::TokenTree;

        fn stream_contains_test(stream: proc_macro2::TokenStream) -> bool {
            let tokens: Vec<TokenTree> = stream.into_iter().collect();
            let mut i = 0;
            while i < tokens.len() {
                match &tokens[i] {
                    TokenTree::Ident(ident) if ident == MODULE_NAME_TEST => return true,
                    TokenTree::Ident(ident) if ident == "not" => {
                        // Skip the `not(...)` group entirely
                        if matches!(tokens.get(i + 1), Some(TokenTree::Group(_))) {
                            i += 2;
                            continue;
                        }
                    }
                    TokenTree::Group(group) => {
                        if stream_contains_test(group.stream()) {
                            return true;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            false
        }

        for attr in attrs {
            if let Meta::List(meta_list) = &attr.meta
                && meta_list.path.is_ident(ATTR_CFG)
                && stream_contains_test(meta_list.tokens.clone())
            {
                return true;
            }
        }
        false
    }

    /// Collects submodules from inline module items.
    fn collect_inline_submodules(
        items: &[Item],
        current_module_path: &str,
        containing_file: &Path,
        base_dir: &Path,
        include_tests: bool,
    ) -> Result<Vec<ModuleInfo>> {
        let mut result = Vec::new();

        for item in items {
            if let Item::Mod(item_mod) = item {
                let mod_name = item_mod.ident.to_string();

                // Check if this is a test module
                let is_test_module = Self::has_cfg_test(&item_mod.attrs);

                // Skip test modules if not including them
                if is_test_module && !include_tests {
                    continue;
                }

                let submodule_path = format!("{current_module_path}::{mod_name}");

                if let Some((_, nested_items)) = &item_mod.content {
                    // Inline module - record with containing file and recurse into items
                    result.push(ModuleInfo::new(
                        submodule_path.clone(),
                        containing_file.to_path_buf(),
                    ));
                    result.extend(Self::collect_inline_submodules(
                        nested_items,
                        &submodule_path,
                        containing_file,
                        base_dir,
                        include_tests,
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
                            &[], // Empty inline scope for file-based modules
                            include_tests,
                        )?);
                    }
                }
            }
        }

        Ok(result)
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

    /// Reads and parses a Rust source file.
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
