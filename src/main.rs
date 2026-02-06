use clap::Parser;
use crawk::cli::{CrawkArgs, CrawkCommands};
use crawk::collector::collect_use_statements;
use crawk::resolver::find_module_by_path;
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use tracing::{Level, error, info};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

struct MinimalFormat;

impl<S, N> FormatEvent<S, N> for MinimalFormat
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        let level = *event.metadata().level();
        let colored_level = match level {
            Level::ERROR => level.as_str().red().to_string(),
            Level::WARN => level.as_str().yellow().to_string(),
            Level::INFO => level.as_str().green().to_string(),
            Level::DEBUG => level.as_str().blue().to_string(),
            Level::TRACE => level.as_str().purple().to_string(),
        };
        write!(writer, "{colored_level} ")?;
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

fn main() {
    // Parse command-line arguments
    let command = CrawkArgs::parse();

    // Initialize tracing subscriber based on verbose flag
    let level = if command.verbose() {
        LevelFilter::DEBUG
    } else {
        LevelFilter::WARN
    };
    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .event_format(MinimalFormat)
        .init();

    // Get crate root and validate it exists
    let crate_root = command.crate_root();

    // Dispatch to the appropriate subcommand
    match command.command {
        CrawkCommands::Use(ref args) => handle_use_command(&crate_root, args),
    }
}

/// Handle the 'use' subcommand
fn handle_use_command(crate_root: &Path, args: &crawk::cli::UseArgs) {
    let src_dir = crate_root.join("src");

    if !src_dir.exists() {
        error!(
            "Not a Rust project directory (src/ not found in {})",
            crate_root.display()
        );
        std::process::exit(1);
    }

    // Parse the module path into components
    let module_components = args.module_components();

    // Find the module file by navigating through the module hierarchy
    let Some(module_file_path) = find_module_by_path(&src_dir, &module_components) else {
        error!("Module '{}' not found", args.module_path);
        std::process::exit(1);
    };

    info!("Crate root: {}", crate_root.display());
    info!("Analyzing module: {}", args.module_path);
    info!("Module file: {}", module_file_path.display());
    if !args.include_tests {
        info!("(excluding tests - use --include-tests to include them)");
    }

    // Collect all use statements from the module and its submodules
    let mut use_statements = HashSet::new();
    collect_use_statements(
        &module_file_path,
        &mut use_statements,
        args.include_tests,
        &module_components,
        args.expand,
        args.depth,
    );

    // Output results
    if use_statements.is_empty() {
        info!("No internal crate use statements found.");
    } else {
        let mut sorted_uses: Vec<_> = use_statements.into_iter().collect();
        sorted_uses.sort();
        for use_stmt in sorted_uses {
            println!("{use_stmt}");
        }
    }
}
