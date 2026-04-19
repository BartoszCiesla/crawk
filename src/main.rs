mod cli;
mod format;
mod logger;

use clap::Parser;
use cli::{CrawkArgs, CrawkCommands, ListArgs, ListOutputFormat, UseArgs, UseOutputFormat};
use crawk::{AnalysisOptions, Analyzer, version};
use logger::configure_tracing;
use std::path::Path;
use tracing::info;

fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let command = CrawkArgs::parse();

    // Configure logging based on command-line options
    configure_tracing(&command)?;

    // Get crate root directory
    let crate_root = command.crate_root()?;

    info!("Running {} v{}", version::NAME, version::VERSION);
    info!("Crate root: {}", crate_root.display());

    // Dispatch to the appropriate subcommand
    match command.command {
        CrawkCommands::Use(ref args) => handle_use_command(&crate_root, args)?,
        CrawkCommands::List(ref args) => handle_list_command(&crate_root, args)?,
    }

    Ok(())
}

/// Handle the 'list' subcommand
fn handle_list_command(crate_root: &Path, args: &ListArgs) -> anyhow::Result<()> {
    let mut analyzer = Analyzer::new(crate_root)?;

    let module_path = args.module_path.as_deref().unwrap_or("lib");
    info!("Listing modules from: {module_path}");

    let mut modules = analyzer.list_modules(module_path, args.include_tests)?;

    // Filter out the crate root (empty path)
    modules.retain(|m| !m.path().is_empty());

    // Apply depth filter
    if let Some(depth) = args.depth {
        modules.retain(|m| m.path().matches("::").count() < depth);
    }

    // Apply substring filter
    if let Some(ref filter) = args.filter {
        modules.retain(|m| m.path().contains(filter.as_str()));
    }

    if modules.is_empty() {
        info!("No modules found.");
    } else {
        let output = match args.format {
            ListOutputFormat::Plain => {
                format::list::render_list_plain(&modules, args.source, crate_root)
            }
            ListOutputFormat::Table => {
                format::list::render_list_table(&modules, args.source, crate_root)
            }
        };
        print!("{output}");
    }

    Ok(())
}

/// Handle the 'use' subcommand
fn handle_use_command(crate_root: &Path, args: &UseArgs) -> anyhow::Result<()> {
    // Create analyzer and validate crate root
    let mut analyzer = Analyzer::new(crate_root)?;

    // Log the module being analyzed
    info!("Analyzing module: {}", args.module_path);

    // Configure analysis options
    let options = AnalysisOptions {
        recursive: args.recursive,
        include_tests: args.include_tests,
        expand_groups: args.expand,
        resolve_globs: args.resolve_globs,
    };

    // Analyze the module
    let result = analyzer.analyze_module(&args.module_path, &options)?;

    // Log the source file of the analyzed module
    info!("Module file: {}", result.source_file().display());

    if result.is_empty() {
        info!("No internal crate use statements found.");
    } else {
        let output = match args.format {
            UseOutputFormat::Plain => format::flat::render_flat(&result, args.depth),
            UseOutputFormat::Grouped => format::grouped::render_grouped(&result, args.depth),
        };
        print!("{output}");
    }

    Ok(())
}
