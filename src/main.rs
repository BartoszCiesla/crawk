mod cli;

use clap::Parser;
use cli::{CrawkArgs, CrawkCommands, UseArgs};
use crawk::{AnalysisOptions, Analyzer, version};
use owo_colors::OwoColorize;
use std::fmt::Result;
use std::fs::File;
use std::path::Path;
use std::process::exit;
use tracing::{Level, Subscriber, error, info};
use tracing_subscriber::{
    EnvFilter,
    fmt::{
        FmtContext,
        format::{FormatEvent, FormatFields, Writer},
    },
    registry::LookupSpan,
};

struct MinimalFormat;

impl<S, N> FormatEvent<S, N> for MinimalFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> Result {
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

    // Initialize tracing subscriber based on verbosity level and log file option
    if let Some(log_file_path) = command.log_file() {
        let file = File::create(log_file_path).unwrap_or_else(|e| {
            eprintln!(
                "Failed to create log file '{}': {e}",
                log_file_path.display()
            );
            exit(1);
        });

        let filter = EnvFilter::builder()
            .with_default_directive(command.file_verbosity().into())
            .from_env_lossy();
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .init();
    } else {
        let filter = EnvFilter::builder()
            .with_default_directive(command.verbosity().into())
            .from_env_lossy();
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .event_format(MinimalFormat)
            .init();
    }

    // Get crate root and validate it exists
    let crate_root = command.crate_root();

    // Dispatch to the appropriate subcommand
    match command.command {
        CrawkCommands::Use(ref args) => handle_use_command(&crate_root, args),
    }
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
