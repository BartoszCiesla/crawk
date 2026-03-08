use anyhow::Context;
use clap::{ArgAction, Parser, Subcommand};
use crawk::version;
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;

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
    let name = version::NAME;
    let after_help = format!(
        "Run '{name} --help' for full help message.\n\
         Run '{name} COMMAND --help' for more information on a command.\n\n"
    );

    if long_help {
        let timestamp = version::BUILD_TIMESTAMP;
        let timestamp = &timestamp[0..timestamp.rfind('.').unwrap_or(timestamp.len())];
        let target = version::BUILD_TARGET;
        let rustc = version::RUSTC_VERSION;
        let user = version::BUILD_USER;
        let homepage = version::HOMEPAGE;
        let build_info = format!("Built on {timestamp}Z for {target} ({rustc}) by {user}");

        format!(
            "{after_help}For more about the tool head to {homepage}\n\n\
             {build_info}\n"
        )
    } else {
        after_help
    }
}

#[derive(Parser, Debug, Clone)]
#[command(
    version = version::VERSION,
    long_version = version::LONG_VERSION_MESSAGE,
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
    #[clap(flatten)]
    options: CrawkOptions,

    #[command(subcommand)]
    pub command: CrawkCommands,
}

#[derive(Parser, Debug, Clone)]
pub struct CrawkOptions {
    /// Specify path to the crate root directory (defaults to current directory)
    #[arg(short = 'p', long = "path")]
    path: Option<PathBuf>,

    /// Increase output verbosity (-v for info, -vv for debug)
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,

    /// Write logs to specified file (INFO level, or DEBUG with -vv)
    #[arg(short = 'l', long = "log-file")]
    log_file: Option<PathBuf>,
}

impl CrawkArgs {
    /// Get the crate root directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No path is provided and the current directory cannot be determined
    /// - The provided path does not exist
    pub fn crate_root(&self) -> anyhow::Result<PathBuf> {
        match self.options.path.as_ref() {
            Some(path) => {
                if !path.exists() {
                    anyhow::bail!("Provided path '{}' does not exist", path.display());
                }
                Ok(path.clone())
            }
            None => std::env::current_dir().context("Failed to get current directory"),
        }
    }

    /// Get the log level filter based on verbosity
    /// # Returns
    /// LevelFilter: WARN (default), INFO (-v), or DEBUG (-vv)
    #[must_use]
    pub const fn verbosity(&self) -> LevelFilter {
        match self.options.verbose {
            0 => LevelFilter::WARN,
            1 => LevelFilter::INFO,
            _ => LevelFilter::DEBUG,
        }
    }

    /// Get the log file path if specified
    #[must_use]
    pub const fn log_file(&self) -> Option<&PathBuf> {
        self.options.log_file.as_ref()
    }

    /// Get the log level filter for file logging
    /// # Returns
    /// LevelFilter: INFO (default), or DEBUG (-vv)
    #[must_use]
    pub const fn file_verbosity(&self) -> LevelFilter {
        match self.options.verbose {
            0 | 1 => LevelFilter::INFO,
            _ => LevelFilter::DEBUG,
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum CrawkCommands {
    /// Analyze a module and list its internal crate `use` statements
    ///
    /// Inspects the given module's source and reports all `use` paths that
    /// reference other modules within the same crate.
    #[clap(verbatim_doc_comment)]
    Use(UseArgs),
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
pub struct UseArgs {
    /// Module path relative to the crate root (e.g., "utils" or "foo::bar::baz")
    #[clap(verbatim_doc_comment)]
    pub module_path: String,

    /// Recursively analyze all submodules (disabled by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'r', long = "recursive", default_value_t = false)]
    pub recursive: bool,

    /// Include modules defined in `#[cfg(test)]` blocks (excluded by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 't', long = "include-tests", default_value_t = false)]
    pub include_tests: bool,

    /// Expand grouped imports into individual paths
    ///
    /// e.g., a::{x, y} becomes a::x, a::y
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'e', long = "expand", default_value_t = false)]
    pub expand: bool,

    /// Limit displayed module path depth from the crate root
    ///
    /// e.g., --depth 1 shows x, --depth 2 shows x::y
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'd', long = "depth", value_parser = validate_depth)]
    pub depth: Option<usize>,

    /// Group output by source module (disabled by default)
    ///
    /// When enabled, dependencies are displayed grouped by their source module,
    /// showing which module each dependency originates from.
    ///
    /// e.g., foo::bar and foo::baz are shown under the foo module
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'g', long = "grouped", default_value_t = false)]
    pub grouped: bool,

    /// Resolve glob imports (`use crate::foo::*`) to explicit items
    ///
    /// When enabled, glob imports are expanded into the individual items
    /// they resolve to based on the target module's public API.
    ///
    /// e.g., foo::* becomes foo::Bar, foo::Baz
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'G', long = "resolve-globs", default_value_t = false)]
    pub resolve_globs: bool,
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
}
