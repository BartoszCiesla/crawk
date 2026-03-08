mod cli;
mod logger;

use clap::Parser;
use cli::{CrawkArgs, CrawkCommands, UseArgs};
use crawk::{AnalysisOptions, Analyzer, TypeReference, version};
use logger::configure_tracing;
use std::collections::BTreeSet;
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
    }

    Ok(())
}

/// Apply optional depth truncation and deduplicate references, returning sorted results.
fn truncate_and_dedup<'a, I>(refs: I, depth: Option<usize>) -> Vec<String>
where
    I: IntoIterator<Item = &'a TypeReference>,
{
    let truncated: BTreeSet<String> = refs
        .into_iter()
        .map(|r| depth.map_or_else(|| r.to_string(), |d| r.truncate_to_depth(d).to_string()))
        .collect();
    truncated.into_iter().collect()
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
    } else if args.grouped {
        let dependencies = result.dependencies();
        let mut modules = dependencies.keys().cloned().collect::<Vec<_>>();
        modules.sort();

        for module in modules {
            // If module name is empty (crate root), use the original module path
            let display_name = if module.is_empty() {
                result.module_path()
            } else {
                module.as_str()
            };
            println!("{display_name}");
            let refs = truncate_and_dedup(&dependencies[&module], args.depth);
            for reference in refs {
                println!(" - {reference}");
            }
        }
    } else {
        let all_refs = result.into_sorted_vec();
        let refs = truncate_and_dedup(&all_refs, args.depth);
        for reference in refs {
            println!("{reference}");
        }
    }

    Ok(())
}
