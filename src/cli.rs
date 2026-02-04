use crate::consts::{
    BUILD_TARGET, BUILD_TIMESTAMP, BUILD_USER, CARGO_BIN_NAME, CARGO_PKG_HOMEPAGE,
    LONG_VERSION_MESSAGE, SDK_VERSION, VERSION_MESSAGE,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Validates that depth is at least 1
/// Validate that the depth argument is a positive integer
///
/// # Errors
///
/// Returns an error if:
/// - The input is not a valid number
/// - The depth value is less than 1
pub fn validate_depth(s: &str) -> Result<usize, String> {
    let value: usize = s
        .parse()
        .map_err(|_| format!("'{s}' is not a valid number"))?;
    if value < 1 {
        Err(String::from("depth must be at least 1"))
    } else {
        Ok(value)
    }
}

/// Generate after help message
/// # Arguments
/// * `long_help` - Whether to generate the long help message
/// # Returns
/// A formatted after help message string
fn generate_after_help(long_help: bool) -> String {
    let after_help = format!(
        "Run '{CARGO_BIN_NAME} --help' for full help message.\n\
         Run '{CARGO_BIN_NAME} COMMAND --help' for more information on a command.\n\n"
    );

    if long_help {
        let timestamp =
            &BUILD_TIMESTAMP[0..BUILD_TIMESTAMP.rfind('.').unwrap_or(BUILD_TIMESTAMP.len())];
        let build_info =
            format!("Built on {timestamp}Z for {BUILD_TARGET} ({SDK_VERSION}) by {BUILD_USER}");

        format!(
            "{after_help}For more about the tool head to {CARGO_PKG_HOMEPAGE}\n\n\
             {build_info}\n"
        )
    } else {
        after_help
    }
}

#[derive(Parser, Debug, Clone)]
#[command(
    version = VERSION_MESSAGE,
    long_version = LONG_VERSION_MESSAGE,
    after_help = generate_after_help(false),
    after_long_help = generate_after_help(true)
)]
#[clap(verbatim_doc_comment)]
/// Analyze Rust module dependencies and structure
///
/// crawk analyzes your Rust codebase and reveals every module dependency — not
/// just `use` statements, but every type annotation, trait bound, struct literal,
/// and macro invocation that ties your code together.
pub struct CrawkArgs {
    #[command(subcommand)]
    pub command: CrawkCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CrawkCommands {
    /// List internal crate use statements from a module
    Use(UseArgs),
}

#[derive(Parser, Debug, Clone)]
#[command(about = "List internal crate use statements from a module")]
pub struct UseArgs {
    /// Module path to analyze (e.g., "utils" or "foo::bar::baz")
    pub module_path: String,

    /// Include test modules in the analysis
    #[arg(short = 't', long = "include-tests")]
    pub include_tests: bool,

    /// Path to the crate root directory (defaults to current directory)
    #[arg(short = 'p', long = "path")]
    pub path: Option<PathBuf>,

    /// Show verbose output including crate root, module path, and analysis info
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Expand grouped imports into individual paths (e.g., a::b::{x, y} -> a::b::x, a::b::y)
    #[arg(short = 'e', long = "expand")]
    pub expand: bool,

    /// Limit module depth from crate root (e.g., --depth 1 shows crate::x, --depth 2 shows crate::x::y)
    #[arg(short = 'd', long = "depth", value_parser = validate_depth)]
    pub depth: Option<usize>,
}

impl UseArgs {
    /// Parse module path into components
    #[must_use]
    pub fn module_components(&self) -> Vec<String> {
        self.module_path
            .split("::")
            .map(ToString::to_string)
            .collect()
    }

    /// Get the crate root directory
    ///
    /// # Panics
    ///
    /// Panics if the current directory cannot be determined when no path is provided
    #[must_use]
    pub fn crate_root(&self) -> PathBuf {
        self.path.as_ref().map_or_else(
            || {
                std::env::current_dir().unwrap_or_else(|_| {
                    eprintln!("Error: Failed to get current directory");
                    std::process::exit(1);
                })
            },
            |path| {
                if !path.exists() {
                    eprintln!("Error: Provided path '{}' does not exist", path.display());
                    std::process::exit(1);
                }
                path.clone()
            },
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_depth_valid() {
        assert_eq!(validate_depth("1").unwrap(), 1);
        assert_eq!(validate_depth("5").unwrap(), 5);
        assert_eq!(validate_depth("100").unwrap(), 100);
    }

    #[test]
    fn test_validate_depth_zero_rejected() {
        let result = validate_depth("0");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "depth must be at least 1");
    }

    #[test]
    fn test_validate_depth_invalid_number() {
        let result = validate_depth("abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a valid number"));
    }

    #[test]
    fn test_validate_depth_negative() {
        let result = validate_depth("-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_module_components_single() {
        let args = UseArgs {
            module_path: "foo".to_string(),
            include_tests: false,
            path: None,
            verbose: false,
            expand: false,
            depth: None,
        };
        assert_eq!(args.module_components(), vec!["foo"]);
    }

    #[test]
    fn test_module_components_nested() {
        let args = UseArgs {
            module_path: "foo::bar::baz".to_string(),
            include_tests: false,
            path: None,
            verbose: false,
            expand: false,
            depth: None,
        };
        assert_eq!(args.module_components(), vec!["foo", "bar", "baz"]);
    }
}
