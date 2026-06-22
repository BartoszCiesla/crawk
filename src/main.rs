mod cli;
mod format;
mod logger;

use clap::Parser;
use cli::{
    CrawkArgs, CrawkCommands, CyclesMode, DepsArgs, DepsOutputFormat, ListArgs, ListOutputFormat,
    UseArgs, UseOutputFormat, WhyArgs, WhyOutputFormat,
};
use crawk::{
    AnalysisOptions, Analyzer, AnnotatedEdges, DependencyGraph, DependencyGraphOptions, version,
};
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
    // Create the canonical, absolute form of a path with all intermediate
    // components normalized and symbolic links resolved.
    let crate_root = crate_root.canonicalize()?;

    // Dispatch to the appropriate subcommand
    match command.command {
        CrawkCommands::Use(ref args) => handle_use_command(&crate_root, args)?,
        CrawkCommands::List(ref args) => handle_list_command(&crate_root, args)?,
        CrawkCommands::Deps(ref args) => handle_deps_command(&crate_root, args)?,
        CrawkCommands::Why(ref args) => handle_why_command(&crate_root, args)?,
    }

    Ok(())
}

/// Handle the 'deps' subcommand
fn handle_deps_command(crate_root: &Path, args: &DepsArgs) -> anyhow::Result<()> {
    let mut analyzer = Analyzer::new(crate_root)?;

    let mut graph_opts = DependencyGraphOptions::default();
    graph_opts.include_tests = args.include_tests;
    graph_opts.depth = args.depth;
    graph_opts.show_apis = args.show_apis;
    let graph = analyzer.dependency_graph(&graph_opts)?;

    let output = if let Some(ref pair) = args.path {
        render_path_output(&graph, &pair[0], &pair[1], args)?
    } else if args.orphans {
        render_orphans_output(&graph)
    } else if let Some(ref cycles_mode) = args.cycles {
        render_cycles_output(&graph, cycles_mode, args)
    } else {
        render_deps_output(graph.edges(), &args.format)
    };

    if output.is_empty() {
        if args.orphans {
            eprintln!("No orphan modules found.");
        } else if args.cycles.is_some() {
            eprintln!("No dependency cycles found.");
        } else {
            info!("No inter-module dependencies found.");
        }
    } else {
        print!("{output}");
    }

    Ok(())
}

fn render_path_output(
    graph: &DependencyGraph,
    src: &str,
    tgt: &str,
    args: &DepsArgs,
) -> anyhow::Result<String> {
    let sp = graph.shortest_paths(src, tgt)?;
    if sp.is_empty() {
        eprintln!("No path from {src} to {tgt}.");
        return Ok(String::new());
    }
    info!(
        "Found {} shortest path(s) of length {}.",
        sp.paths.len(),
        sp.length().unwrap_or(0)
    );
    Ok(match args.format {
        DepsOutputFormat::Plain => format::paths::render_paths_plain(&sp, args.depth),
        DepsOutputFormat::Grouped => format::paths::render_paths_grouped(&sp, args.depth),
        DepsOutputFormat::Dot => format::paths::render_paths_dot(graph.edges(), &sp, args.depth),
    })
}

fn render_orphans_output(graph: &DependencyGraph) -> String {
    let orphans = graph.orphans();
    if orphans.is_empty() {
        return String::new();
    }
    info!("Found {} orphan module(s).", orphans.len());
    format::orphans::render_orphans(&orphans)
}

fn render_cycles_output(
    graph: &DependencyGraph,
    cycles_mode: &CyclesMode,
    args: &DepsArgs,
) -> String {
    let cycles = graph.cycles();
    if cycles.is_empty() {
        return String::new();
    }
    info!("Found {} dependency cycle(s).", cycles.len());
    if *cycles_mode == CyclesMode::Highlight && args.format != DepsOutputFormat::Dot {
        eprintln!(
            "warning: --cycles highlight has no effect with {} format, showing cycles only",
            args.format
        );
    }
    match (&args.format, cycles_mode) {
        (DepsOutputFormat::Plain, _) => format::cycles::render_cycles_plain(&cycles),
        (DepsOutputFormat::Grouped, _) => format::cycles::render_cycles_grouped(&cycles),
        (DepsOutputFormat::Dot, CyclesMode::Detect) => format::cycles::render_cycles_dot(&cycles),
        (DepsOutputFormat::Dot, CyclesMode::Highlight) => {
            format::cycles::render_cycles_dot_highlight(&cycles, graph.edges())
        }
    }
}

fn render_deps_output(edges: &AnnotatedEdges, format: &DepsOutputFormat) -> String {
    match format {
        DepsOutputFormat::Plain => format::deps_cmd::render_plain(edges),
        DepsOutputFormat::Grouped => format::deps_cmd::render_grouped(edges),
        DepsOutputFormat::Dot => format::deps_cmd::render_dot(edges),
    }
}

/// Handle the 'list' subcommand
fn handle_list_command(crate_root: &Path, args: &ListArgs) -> anyhow::Result<()> {
    let mut analyzer = Analyzer::new(crate_root)?;

    let (mut modules, is_all_targets) = if let Some(ref module_path) = args.module_path {
        // Single-target context: list subtree from the given module
        info!("Listing modules from: {module_path}");
        let mods = analyzer.list_modules(module_path, args.include_tests)?;
        (mods, false)
    } else {
        // Multi-target context: list all targets
        info!("Listing all targets");
        let mods = analyzer.list_all_modules(args.include_tests)?;
        (mods, true)
    };

    // Show target prefix when forced, or when multiple distinct targets have modules
    let multi_target = args.display.show_targets || {
        is_all_targets && {
            let distinct_targets = modules
                .iter()
                .map(crawk::ModuleInfo::target)
                .collect::<std::collections::HashSet<_>>()
                .len();
            distinct_targets > 1
        }
    };

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
        let display_opts = format::list_cmd::ListDisplayOptions {
            show_source: args.display.show_source,
            show_visibility: args.display.show_visibility,
            multi_target,
        };
        let output = match args.format {
            ListOutputFormat::Plain => {
                format::list_cmd::render_list_plain(&modules, &display_opts, crate_root)
            }
            ListOutputFormat::Table => {
                format::list_cmd::render_list_table(&modules, &display_opts, crate_root)
            }
        };
        print!("{output}");
    }

    Ok(())
}

/// Handle the 'why' subcommand
fn handle_why_command(crate_root: &Path, args: &WhyArgs) -> anyhow::Result<()> {
    let mut analyzer = Analyzer::new(crate_root)?;
    let options = AnalysisOptions {
        recursive: args.recursive,
        include_tests: args.include_tests,
        expand_groups: true,
        resolve_globs: false,
    };
    let refs = analyzer.explain_dependency(&args.source, &args.target, &options)?;

    if refs.is_empty() {
        info!("No references from '{}' to '{}'.", args.source, args.target);
    } else {
        let output = match args.format {
            WhyOutputFormat::Plain => format::why_cmd::render_plain(&refs),
            WhyOutputFormat::Grouped => format::why_cmd::render_grouped(&refs),
        };
        print!("{output}");
    }

    Ok(())
}

/// Handle the 'use' subcommand
fn handle_use_command(crate_root: &Path, args: &UseArgs) -> anyhow::Result<()> {
    // Create analyzer and validate crate root
    let mut analyzer = Analyzer::new(crate_root)?;

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
            UseOutputFormat::Plain => format::use_cmd::render_flat(&result, args.depth),
            UseOutputFormat::Grouped => format::use_cmd::render_grouped(&result, args.depth),
        };
        print!("{output}");
    }

    Ok(())
}
