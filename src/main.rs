use clap::Parser;
use crawk::cli::{Cargo, ModuleCommand, ModuleCommands};
use crawk::collector::collect_use_statements;
use crawk::resolver::find_module_by_path;
use std::collections::HashSet;
use std::env;

fn main() {
    // Parse command-line arguments
    let command = parse_command();

    // Dispatch to the appropriate subcommand
    match command.command {
        ModuleCommands::Use(args) => handle_use_command(args),
    }
}

/// Handle the 'use' subcommand
fn handle_use_command(args: crawk::cli::UseArgs) {
    // Get crate root and validate it exists
    let crate_root = args.crate_root();
    let src_dir = crate_root.join("src");

    if !src_dir.exists() {
        eprintln!(
            "Error: Not a Rust project directory (src/ not found in {})",
            crate_root.display()
        );
        std::process::exit(1);
    }

    // Parse the module path into components
    let module_components = args.module_components();

    // Find the module file by navigating through the module hierarchy
    let module_file_path = match find_module_by_path(&src_dir, &module_components) {
        Some(path) => path,
        None => {
            eprintln!("Error: Module '{}' not found", args.module_path);
            std::process::exit(1);
        }
    };

    // Print verbose information if requested
    if args.verbose {
        println!("Crate root: {}", crate_root.display());
        println!("Analyzing module: {}", args.module_path);
        println!("Module file: {}", module_file_path.display());
        if !args.include_tests {
            println!("(excluding tests - use --include-tests to include them)");
        }
        println!();
    }

    // Collect all use statements from the module and its submodules
    let mut use_statements = HashSet::new();
    collect_use_statements(
        &module_file_path,
        &mut use_statements,
        args.include_tests,
        args.verbose,
        &module_components,
        args.expand,
        args.depth,
    );

    // Output results
    if use_statements.is_empty() {
        if args.verbose {
            println!("No internal crate use statements found.");
        }
    } else {
        let mut sorted_uses: Vec<_> = use_statements.into_iter().collect();
        sorted_uses.sort();
        for use_stmt in sorted_uses {
            println!("{}", use_stmt);
        }
    }
}

/// Parse command-line arguments, handling both cargo subcommand and standalone invocations
fn parse_command() -> ModuleCommand {
    if env::args().nth(1).as_deref() == Some("module") {
        // Invoked as: cargo module <subcommand> <args>
        match Cargo::try_parse() {
            Ok(Cargo::Module(cmd)) => cmd,
            Err(e) => {
                e.exit();
            }
        }
    } else {
        // Invoked directly as: crawk <subcommand> <args>
        ModuleCommand::parse()
    }
}
