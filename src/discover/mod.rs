//! Module for locating and resolving Rust module paths within a crate.
//!
//! This module provides the [`CrateInfo`] struct which wraps cargo metadata
//! and provides functionality to resolve module paths (like `analysis::collect`)
//! to their corresponding file paths on disk.

mod module_tree;

use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, MetadataCommand};
use thiserror::Error;

/// Errors that can occur during crate info operations.
#[derive(Debug, Error)]
pub enum CrateInfoError {
    /// Failed to execute cargo metadata command.
    #[error("Failed to execute cargo metadata: {0}")]
    MetadataExecution(#[from] cargo_metadata::Error),

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
            .ok_or(CrateInfoError::WorkspaceRoot)?
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

    /// Returns the root package from cargo metadata.
    fn root_package(&self) -> Option<&cargo_metadata::Package> {
        self.metadata
            .packages
            .iter()
            .find(|p| p.name == self.root_package_name)
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

        // Determine if this is an inline module and compute inline scope
        let inline_scope = self.compute_inline_scope_for_path(&normalized_path, &file_path);

        if recursive {
            Self::collect_submodules_recursive(
                &file_path,
                &normalized_path,
                &inline_scope,
                include_tests,
            )
        } else {
            Self::collect_submodules_shallow(&file_path, &normalized_path, include_tests)
        }
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

    /// Normalizes a module path by removing the crate name prefix if present.
    /// Also normalizes "lib" alias to the crate name.
    /// Also normalizes binary target file stems (e.g., "main" for main.rs).
    /// If the module path is just the crate name/lib/binary, returns empty string.
    fn normalize_module_path(&self, module_path: &str) -> String {
        let parts: Vec<&str> = module_path.split("::").collect();
        if parts.is_empty() {
            return module_path.to_string();
        }

        let first_part = parts[0];

        // Check if first part is the crate name or "lib" alias
        if first_part == self.root_package_name() || first_part == "lib" {
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

        module_path.to_string()
    }
}
