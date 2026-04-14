use crate::discover::CrateInfoError;
use crate::parser::AnalyzerError;
use std::path::PathBuf;
use thiserror::Error;

/// Error types for analysis operations.
#[derive(Debug, Error)]
#[non_exhaustive]
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

    /// Error analyzing a specific module — includes module name and file for context.
    #[error("Error analyzing module '{module_path}' (file '{file}'): {source}")]
    ModuleAnalysisFailed {
        /// The Rust module path being analyzed (e.g. `crate::parser::visitor`).
        module_path: String,
        /// The source file being parsed.
        file: PathBuf,
        /// The underlying parser error — inspect [`AnalyzerError`] variants for details.
        source: AnalyzerError,
    },
}

/// Result type alias for analysis info operations.
pub type Result<T> = std::result::Result<T, AnalysisError>;
