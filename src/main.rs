mod cli;
mod format;
mod logger;

use clap::Parser;
use cli::{
    CrawkArgs, CrawkCommands, DepsArgs, DepsOutputFormat, ListArgs, ListOutputFormat, UseArgs,
    UseOutputFormat,
};
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
    // Create the canonical, absolute form of a path with all intermediate
    // components normalized and symbolic links resolved.
    let crate_root = crate_root.canonicalize()?;

    // Dispatch to the appropriate subcommand
    match command.command {
        CrawkCommands::Use(ref args) => handle_use_command(&crate_root, args)?,
        CrawkCommands::List(ref args) => handle_list_command(&crate_root, args)?,
        CrawkCommands::Deps(ref args) => handle_deps_command(&crate_root, args)?,
    }

    Ok(())
}

/// Handle the 'deps' subcommand
fn handle_deps_command(crate_root: &Path, args: &DepsArgs) -> anyhow::Result<()> {
    let mut analyzer = Analyzer::new(crate_root)?;

    // Always recursive and with groups expanded so every reference is a plain
    // path whose segments directly address the target module and item.
    let options = AnalysisOptions {
        recursive: true,
        include_tests: args.include_tests,
        expand_groups: true,
        resolve_globs: false,
    };

    // Discover all compilation targets. list_all_modules already respects
    // include_tests, so integration test targets are only present when -t is set.
    let all_modules = analyzer.list_all_modules(args.include_tests)?;

    let roots = collect_target_roots(&all_modules);
    info!("Building dependency graph across {} target(s)", roots.len());

    // Build a lookup set of every known module path across all targets.
    // build_edges uses this to resolve TypeReference segments to their owning
    // module (stripping trailing item names like types, functions, constants).
    let known_modules: std::collections::HashSet<String> =
        all_modules.iter().map(|m| m.path().to_owned()).collect();

    // Package name is used to recognise cross-target references from binaries
    // or integration tests to the lib target (e.g. `crawk::Analyzer`).
    let package_name: Option<String> = all_modules
        .iter()
        .find(|m| m.target().kind() == &crawk::TargetKind::Lib)
        .map(|m| m.target().name().to_owned());

    let mut all_edges = std::collections::BTreeSet::new();
    for root in &roots {
        info!("Analysing target root '{root}'");
        match analyzer.analyze_module(root.as_str(), &options) {
            Ok(result) => {
                all_edges.extend(format::deps::build_edges(
                    &result,
                    args.depth,
                    &known_modules,
                    package_name.as_deref(),
                ));
            }
            Err(e) => info!("Skipping target '{root}': {e}"),
        }
    }

    if all_edges.is_empty() {
        info!("No inter-module dependencies found.");
    } else {
        let output = match args.format {
            DepsOutputFormat::Plain => format::deps::render_plain(&all_edges),
            DepsOutputFormat::Grouped => format::deps::render_grouped(&all_edges),
        };
        print!("{output}");
    }

    Ok(())
}

/// Determine the root module path for each unique compilation target.
///
/// For lib targets the root is always `"lib"`. For binary and test targets the
/// root is identified from the module's source file path (e.g. `src/main.rs`
/// → `"main"`, `src/bin/foo.rs` → `"foo"`). If the heuristic cannot identify
/// the entry point, the lexicographically smallest top-level module is used as
/// a fallback.
fn collect_target_roots(modules: &[crawk::ModuleInfo]) -> Vec<String> {
    use std::collections::HashMap;

    // Group modules by (kind, name).
    let mut groups: HashMap<(crawk::TargetKind, String), Vec<&crawk::ModuleInfo>> = HashMap::new();
    for m in modules {
        let key = (m.target().kind().clone(), m.target().name().to_owned());
        groups.entry(key).or_default().push(m);
    }

    // Process in stable order: lib first, then bins, then tests (alphabetically).
    let mut keys: Vec<_> = groups.keys().cloned().collect();
    keys.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut roots = Vec::new();
    for (kind, name) in &keys {
        let group = &groups[&(kind.clone(), name.clone())];
        let root = match kind {
            crawk::TargetKind::Lib => "lib".to_owned(),
            crawk::TargetKind::Bin | crawk::TargetKind::Test => {
                find_bin_or_test_root(group).unwrap_or_else(|| name.clone())
            }
        };
        roots.push(root);
    }
    roots
}

/// Identify the root module path for a binary or integration-test target.
///
/// `list_all_modules` renames the root module (originally the empty path `""`)
/// to the canonical name — the file stem of the entry-point source file (e.g.
/// `src/main.rs` → `"main"`). This function recovers that path by looking for
/// a top-level module whose source file matches known cargo entry-point patterns:
///
/// - `src/main.rs` (default binary)
/// - Any file inside `src/bin/` (named binaries)
/// - Any file inside `tests/` (integration test targets)
///
/// Falls back to the lexicographically smallest top-level module when no
/// pattern matches (covers `[[bin]] path = "src/custom.rs"` in Cargo.toml).
fn find_bin_or_test_root(modules: &[&crawk::ModuleInfo]) -> Option<String> {
    // Only consider modules without `::` (top-level candidates).
    let top_level: Vec<_> = modules
        .iter()
        .filter(|m| !m.path().contains("::"))
        .collect();

    // Prefer the module whose source file is a recognised cargo entry point.
    let preferred = top_level.iter().find(|m| {
        let src = m.source();
        src.file_name().is_some_and(|n| n == "main.rs")
            || src.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::Normal(n) if n == "bin" || n == "tests"
                )
            })
    });

    preferred
        .or_else(|| top_level.iter().min_by_key(|m| m.path()))
        .map(|m| m.path().to_owned())
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
        let display_opts = format::list::ListDisplayOptions {
            show_source: args.display.show_source,
            show_visibility: args.display.show_visibility,
            multi_target,
        };
        let output = match args.format {
            ListOutputFormat::Plain => {
                format::list::render_list_plain(&modules, &display_opts, crate_root)
            }
            ListOutputFormat::Table => {
                format::list::render_list_table(&modules, &display_opts, crate_root)
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
