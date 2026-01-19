use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Validates that depth is at least 1
pub fn validate_depth(s: &str) -> Result<usize, String> {
    let value: usize = s
        .parse()
        .map_err(|_| format!("'{}' is not a valid number", s))?;
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
    pub fn module_components(&self) -> Vec<String> {
        self.module_path
            .split("::")
            .map(|s| s.to_string())
            .collect()
    }

    /// Get the crate root directory
    pub fn crate_root(&self) -> PathBuf {
        if let Some(path) = &self.path {
            if !path.exists() {
                eprintln!("Error: Provided path '{}' does not exist", path.display());
                std::process::exit(1);
            }
            path.clone()
        } else {
            std::env::current_dir().expect("Failed to get current directory")
        }
    }
}
