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

#[derive(Parser, Debug)]
#[command(
    name = "crawk",
    bin_name = "cargo",
    about = "Analyze Rust module dependencies and structure"
)]
pub enum Cargo {
    #[command(name = "module", subcommand_required = true)]
    Module(ModuleCommand),
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Analyze Rust module dependencies and structure")]
pub struct ModuleCommand {
    #[command(subcommand)]
    pub command: ModuleCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ModuleCommands {
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
