mod cli;
mod format;
mod logger;

use clap::Parser;
use cli::{
    CheckArgs, CheckOutputFormat, CrawkArgs, CrawkCommands, CyclesMode, DepsArgs, DepsOutputFormat,
    ListArgs, ListOutputFormat, UseArgs, UseOutputFormat, WhyArgs, WhyOutputFormat,
};
use crawk::{
    AnalysisOptions, Analyzer, AnnotatedEdges, CheckOptions, DependencyGraph,
    DependencyGraphOptions, version,
};
use logger::configure_tracing;
use std::path::Path;
use tracing::info;

fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let command = CrawkArgs::parse();

    // The check command owns a distinct exit-code contract: 0 clean, 1
    // violations, 2 operational error. Every failure on its path — including
    // the setup steps before dispatch — maps to 2 rather than letting anyhow
    // return the default exit code 1 from `main`.
    let is_check = matches!(command.command, CrawkCommands::Check(_));

    match run(&command) {
        Ok(0) => Ok(()),
        Ok(code) => exit_with(code),
        Err(e) if is_check => {
            eprintln!("Error: {e:#}");
            exit_with(2)
        }
        Err(e) => Err(e),
    }
}

/// Sole audited `process::exit`: the check command's distinct exit-code
/// contract (0 clean / 1 violations / 2 operational).
fn exit_with(code: i32) -> ! {
    #[allow(clippy::exit)]
    std::process::exit(code)
}

/// Run the requested command. Returns the intended process exit code.
fn run(command: &CrawkArgs) -> anyhow::Result<i32> {
    // Configure logging based on command-line options
    configure_tracing(command)?;

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
        CrawkCommands::Check(ref args) => return handle_check_command(&crate_root, args),
    }

    Ok(0)
}

/// Handle the 'check' subcommand. Returns the intended process exit code:
/// `0` (clean) or `1` (violations). Operational errors propagate as `Err`.
fn handle_check_command(crate_root: &Path, args: &CheckArgs) -> anyhow::Result<i32> {
    let mut analyzer = Analyzer::new(crate_root)?;

    let opts = CheckOptions {
        config: args.config.clone(),
        include_tests: args.include_tests,
        show_apis: args.show_apis,
    };

    if args.init {
        let path = analyzer.init_check_config(crate_root, &opts)?;
        // A custom `-c` path is not auto-discovered, so the re-run hint must
        // carry it; the default crawk.toml is found by a plain `crawk check`.
        let rerun = args.config.as_ref().map_or_else(
            || "crawk check".to_owned(),
            |cfg| format!("crawk check -c {}", cfg.display()),
        );
        eprintln!("Scaffolded {}.", path.display());
        eprintln!();
        eprintln!("  Reorder the modules: highest-level layer first, lowest last.");
        eprintln!("  A lower layer must never depend on a higher one.");
        eprintln!("  Then run `{rerun}`.");
        return Ok(0);
    }

    let report = analyzer.check(crate_root, &opts)?;

    if report.is_clean() {
        info!("All architectural rules satisfied.");
        return Ok(0);
    }

    let output = match args.format {
        CheckOutputFormat::Plain => format::check_cmd::render_plain(&report),
    };
    print!("{output}");
    Ok(report.exit_code())
}

/// Handle the 'deps' subcommand
fn handle_deps_command(crate_root: &Path, args: &DepsArgs) -> anyhow::Result<()> {
    let mut analyzer = Analyzer::new(crate_root)?;

    let mut graph_opts = DependencyGraphOptions::default();
    graph_opts.include_tests = args.include_tests;
    // `--path` resolves SOURCE/TARGET against the full-granularity module set;
    // for that combination `--depth` is a render-time concern only, applied by
    // `format::paths`. Truncating the graph first would reject valid module
    // paths that survive only in their full form.
    graph_opts.depth = if args.path.is_some() {
        None
    } else {
        args.depth
    };
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
    // `--depth` is applied here, by the library: `format::paths` renders what
    // it is given and has no truncation logic of its own.
    let resolved = sp.truncated(args.depth);
    Ok(match args.format {
        DepsOutputFormat::Plain => format::paths::render_paths_plain(&resolved),
        DepsOutputFormat::Grouped => format::paths::render_paths_grouped(&resolved),
        DepsOutputFormat::Dot => {
            format::paths::render_paths_dot(&graph.truncated_edges(args.depth), &resolved)
        }
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
