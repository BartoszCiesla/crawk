use crate::discover::CrateInfoError;
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

    /// A bare module path names several modules, each under its own target.
    #[error("Module '{module_path}' is ambiguous, pick one of: {}", .candidates.join(", "))]
    AmbiguousModule {
        /// The bare path as given.
        module_path: String,
        /// Full paths of the modules it could mean: `lib::x`, `bin@<name>::x`,
        /// `test@<name>::x`.
        candidates: Vec<String>,
    },

    /// A path is led by the file stem of a binary or test, which does not name it.
    #[error(
        "Module '{module_path}' not found: a binary or test goes by its cargo target name, did you mean {}?",
        .candidates.join(", ")
    )]
    TargetFileStem {
        /// The path as given.
        module_path: String,
        /// The same path under each binary or test with that file stem.
        candidates: Vec<String>,
    },

    /// The crate root directory does not exist or is not a valid Rust project.
    #[error("Invalid crate root: {path} - {reason}")]
    InvalidCrateRoot {
        /// The path that was provided.
        path: PathBuf,
        /// Description of what's wrong.
        reason: String,
    },

    /// Errors related to crate metadata retrieval and validation; see [`CrateInfoError`].
    #[error(transparent)]
    CrateInfoError(#[from] CrateInfoError),

    /// Error analyzing a specific module — includes module name and file for context.
    #[error("Error analyzing module '{module_path}' (file '{file}'): {message}")]
    ModuleAnalysisFailed {
        /// The Rust module path being analyzed (e.g. `crate::parser::visitor`).
        module_path: String,
        /// The source file being parsed.
        file: PathBuf,
        /// Human-readable description of the parse or read error.
        message: String,
    },

    /// The rule configuration file could not be found, read, or parsed.
    #[error("Invalid rule config ({path}): {reason}")]
    RuleConfigError {
        /// The configuration file path involved.
        path: PathBuf,
        /// Human-readable description of what went wrong.
        reason: String,
    },

    /// A rule references a module that does not exist in the crate.
    #[error(
        "Rule references unknown module '{module}' (in rule '{rule}'){}",
        did_you_mean(.candidates)
    )]
    UnknownRuleModule {
        /// The unknown module path named in the rule.
        module: String,
        /// Human-readable description of the offending rule.
        rule: String,
        /// Full names of binary or test modules the rule may have meant; rules
        /// take no bare form of those.
        candidates: Vec<String>,
    },
}

impl AnalysisError {
    /// Returns `true` if the error reports a module that was not found.
    pub(crate) const fn is_module_not_found(&self) -> bool {
        match self {
            Self::ModuleNotFound { .. } => true,
            Self::CrateInfoError(error) => error.is_module_not_found(),
            _ => false,
        }
    }
}

/// The suffix naming what an unknown rule module may have meant, if anything.
fn did_you_mean(candidates: &[String]) -> String {
    if candidates.is_empty() {
        String::new()
    } else {
        format!(": did you mean {}?", candidates.join(", "))
    }
}

/// Result type alias for analysis info operations.
pub(crate) type Result<T> = std::result::Result<T, AnalysisError>;
