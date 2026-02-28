#![allow(dead_code)]
//! Module for locating and resolving Rust module paths within a crate.
//!
//! This module provides the [`CrateInfo`] struct which wraps cargo metadata
//! and provides functionality to resolve module paths (like `analysis::collect`)
//! to their corresponding file paths on disk.

use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, MetadataCommand, Package};
use syn::{Attribute, Item, Meta};
use thiserror::Error;

use crate::constants::{
    ATTR_CFG, LIB_FILE_NAME, MAIN_FILE_NAME, MODULE_FILE_NAME, MODULE_NAME_TEST,
};

/// Errors that can occur during crate info operations.
#[derive(Debug, Error)]
pub enum CrateInfoError {
    /// Failed to execute cargo metadata command.
    #[error("Failed to execute cargo metadata: {0}")]
    MetadataExecution(#[from] cargo_metadata::Error),

    /// No root package found in the workspace.
    #[error("No root package found in the workspace")]
    NoRootPackage,

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

    /// Failed to parse source file.
    #[error("Failed to parse file '{path}': {message}")]
    ParseError {
        /// The path of the file that could not be parsed.
        path: PathBuf,
        /// The parse error message.
        message: String,
    },
}

/// Result type alias for crate info operations.
pub type Result<T> = std::result::Result<T, CrateInfoError>;

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
}

impl ModuleInfo {
    /// Creates a new `ModuleInfo` instance.
    ///
    /// # Arguments
    ///
    /// * `module_path` - The fully qualified module path (e.g., "analysis::collect")
    /// * `source_file` - The file system path where this module is defined
    pub const fn new(module_path: String, source_file: PathBuf) -> Self {
        Self {
            module_path,
            source_file,
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
}

/// A struct that wraps cargo metadata and provides module path resolution.
///
/// This struct holds the metadata for a Rust crate and provides methods to
/// resolve module paths (like `analysis::collect`) to their corresponding
/// file paths on disk.
#[derive(Debug, Clone)]
pub struct CrateInfo {
    /// The cargo metadata for the crate.
    metadata: Metadata,

    /// The name of the root package in the workspace.
    root_package_name: String,
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
    pub fn new(crate_path: &Path) -> Result<Self> {
        let metadata = MetadataCommand::new().current_dir(crate_path).exec()?;

        let root_package_name = metadata
            .root_package()
            .ok_or(CrateInfoError::NoRootPackage)?
            .name
            .to_string();

        Ok(Self {
            metadata,
            root_package_name,
        })
    }

    /// Returns the name of the root package.
    #[must_use]
    pub fn root_package_name(&self) -> &str {
        &self.root_package_name
    }

    /// Returns whether the crate is part of a workspace with multiple members.
    #[must_use]
    pub const fn is_workspace(&self) -> bool {
        self.metadata.workspace_members.len() > 1
    }

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
    fn resolve_module(&self, module_path: &str) -> Result<PathBuf> {
        let package = self
            .metadata
            .packages
            .iter()
            .find(|p| p.name == self.root_package_name)
            .ok_or(CrateInfoError::NoRootPackage)?;

        self.resolve_module_path_with_crate(package, module_path)
    }

    /// Public wrapper around [`resolve_module`](Self::resolve_module).
    ///
    /// Resolves a module path (e.g., `"foo::bar"`) to the corresponding source file.
    ///
    /// # Errors
    ///
    /// Returns an error if the module cannot be found or the crate root is invalid.
    pub fn resolve_module_path_to_file(&self, module_path: &str) -> Result<PathBuf> {
        self.resolve_module(module_path)
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
    /// Also treats "main" as an alias for the main binary target root.
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

        // Check if the first part is "main" alias for the main binary target
        let is_main_root = parts[0] == "main";

        if is_main_root {
            // Find the main binary target and use it as the root
            return if parts.len() > 1 {
                let remaining_path = parts[1..].join("::");
                Self::resolve_module_path_from_main(package, &remaining_path)
            } else {
                // Just "main", return the main binary root
                Self::find_main_binary(package)
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

    /// Finds the main binary target file (main.rs).
    fn find_main_binary(package: &Package) -> Option<PathBuf> {
        for target in &package.targets {
            if target.is_bin() {
                return Some(target.src_path.clone().into());
            }
        }
        None
    }

    /// Resolves a module path relative to the main binary target.
    ///
    /// Similar to `resolve_module_path`, but uses the main binary as the root.
    fn resolve_module_path_from_main(package: &Package, module_path: &str) -> Result<PathBuf> {
        let parts: Vec<&str> = module_path.split("::").collect();

        if parts.is_empty() || module_path.is_empty() {
            return Err(CrateInfoError::EmptyModulePath);
        }

        let main_root = Self::find_main_binary(package)
            .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))?;

        let main_root_dir = main_root
            .parent()
            .ok_or_else(|| CrateInfoError::NoCrateRoot(package.name.to_string()))?;

        Self::resolve_module_parts(main_root_dir, &parts, Some(&main_root))?.ok_or_else(|| {
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
    /// Returns `Ok(Vec<ModuleInfo>)` containing information about each module and its source file.
    /// For inline modules (like `#[cfg(test)] mod tests`), the source file is the containing file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The module path cannot be resolved
    /// - The source file cannot be read
    /// - The source file cannot be parsed
    pub fn get_module_tree(
        &self,
        module_path: &str,
        recursive: bool,
        include_tests: bool,
    ) -> Result<Vec<ModuleInfo>> {
        let file_path = self.resolve_module(module_path)?;

        // Normalize the module path (remove crate name prefix if present)
        let normalized_path = self.normalize_module_path(module_path);

        if recursive {
            Self::collect_submodules_recursive(&file_path, &normalized_path, include_tests)
        } else {
            Self::collect_submodules_shallow(&file_path, &normalized_path, include_tests)
        }
    }

    /// Normalizes a module path by removing the crate name prefix if present.
    /// Also normalizes "lib" alias to the crate name.
    /// If the module path is just the crate name/lib, returns the crate name (not empty).
    fn normalize_module_path(&self, module_path: &str) -> String {
        let parts: Vec<&str> = module_path.split("::").collect();
        if !parts.is_empty() && (parts[0] == self.root_package_name || parts[0] == "lib") {
            if parts.len() == 1 {
                // Just the crate name/lib - keep it as the root identifier
                self.root_package_name.clone()
            } else {
                parts[1..].join("::")
            }
        } else {
            module_path.to_string()
        }
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

    /// Collects only the current module (non-recursive, no submodules).
    ///
    /// When `include_tests` is `true`, also includes the direct test module
    /// (marked with `#[cfg(test)]`) if one exists directly under the given module.
    fn collect_submodules_shallow(
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

    /// Recursively collects all submodules from a file.
    fn collect_submodules_recursive(
        file_path: &Path,
        current_module_path: &str,
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

        // Extract module declarations
        for item in &syntax.items {
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
                        result.extend(Self::collect_submodules_recursive(
                            &sub_mod_file,
                            &submodule_path,
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
                        result.extend(Self::collect_submodules_recursive(
                            &sub_mod_file,
                            &submodule_path,
                            include_tests,
                        )?);
                    }
                }
            }
        }

        Ok(result)
    }
}
