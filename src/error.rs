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

    /// Errors that occur during module parsing and analysis.
    #[error("Error analyzing module: {0}")]
    AnalyzerError(#[from] AnalyzerError),
}

/// Result type alias for analysis info operations.
pub type Result<T> = std::result::Result<T, AnalysisError>;
