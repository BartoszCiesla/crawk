mod cli;
mod logger;

use clap::Parser;
use cli::{CrawkArgs, CrawkCommands, UseArgs};
use crawk::{AnalysisOptions, Analyzer, version};
use logger::configure_tracing;
use std::path::Path;
use std::process::exit;
use tracing::{error, info};

fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let command = CrawkArgs::parse();

    // Configure logging based on command-line options
    configure_tracing(&command)?;

    // Get crate root and validate it exists
    let crate_root = command.crate_root();

    // Dispatch to the appropriate subcommand
    match command.command {
        CrawkCommands::Use(ref args) => handle_use_command(&crate_root, args),
    }

    Ok(())
}

/// Handle the 'use' subcommand
fn handle_use_command(crate_root: &Path, args: &UseArgs) {
    // Create analyzer and validate crate root
    let analyzer = Analyzer::new(crate_root);

    if let Err(e) = analyzer.validate() {
        error!("{e}");
        exit(1);
    }

    // Parse the module path into components
    let module_components = args.module_components();

    info!("Running {} v{}", version::NAME, version::VERSION);
    info!("Crate root: {}", crate_root.display());
    info!("Analyzing module: {}", args.module_path);

    // Configure analysis options
    let options = AnalysisOptions {
        include_tests: args.include_tests,
        expand_groups: args.expand,
        max_depth: args.depth,
    };

    // Analyze the module
    let result = match analyzer.analyze_module(&module_components, &options) {
        Ok(result) => result,
        Err(e) => {
            error!("{e}");
            exit(1);
        }
    };

    info!("Module file: {}", result.source_file().display());

    // Output results
    if result.is_empty() {
        info!("No internal crate use statements found.");
    } else {
        for use_stmt in result.into_sorted_vec() {
            println!("{use_stmt}");
        }
    }
}
