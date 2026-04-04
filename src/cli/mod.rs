mod overview;
mod validation;

use anyhow::Context;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use crawk::version;
use overview::generate_after_help;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;
use validation::{validate_depth, validate_module_path};

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
pub(crate) struct CrawkArgs {
    #[clap(flatten)]
    options: CrawkOptions,

    #[command(subcommand)]
    pub command: CrawkCommands,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct CrawkOptions {
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
    pub(crate) fn crate_root(&self) -> anyhow::Result<PathBuf> {
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
    pub(crate) const fn verbosity(&self) -> LevelFilter {
        match self.options.verbose {
            0 => LevelFilter::WARN,
            1 => LevelFilter::INFO,
            _ => LevelFilter::DEBUG,
        }
    }

    /// Get the log file path if specified
    #[must_use]
    pub(crate) const fn log_file(&self) -> Option<&PathBuf> {
        self.options.log_file.as_ref()
    }

    /// Get the log level filter for file logging
    /// # Returns
    /// LevelFilter: INFO (default), or DEBUG (-vv)
    #[must_use]
    pub(crate) const fn file_verbosity(&self) -> LevelFilter {
        match self.options.verbose {
            0 | 1 => LevelFilter::INFO,
            _ => LevelFilter::DEBUG,
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CrawkCommands {
    /// Analyze a module and list its internal crate `use` statements
    ///
    /// Inspects the given module's source and reports all `use` paths that
    /// reference other modules within the same crate.
    #[clap(verbatim_doc_comment)]
    Use(UseArgs),
}

#[derive(ValueEnum, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum UseOutputFormat {
    /// Flat sorted list (default)
    #[default]
    Plain,
    /// Grouped by source module
    Grouped,
}

impl Display for UseOutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Plain => f.write_str("plain"),
            Self::Grouped => f.write_str("grouped"),
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
pub(crate) struct UseArgs {
    /// Module path relative to the crate root (e.g., "utils" or "foo::bar::baz")
    #[clap(verbatim_doc_comment)]
    #[arg(value_parser = validate_module_path)]
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

    /// Output format
    ///
    /// plain   — flat sorted list (default)
    /// grouped — grouped by source module
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'f', long = "format", default_value_t = UseOutputFormat::Plain)]
    pub format: UseOutputFormat,

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
