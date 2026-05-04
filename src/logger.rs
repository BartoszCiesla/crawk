use super::cli::CrawkArgs;
use anyhow::Context;
use owo_colors::OwoColorize;
use std::fmt::Result as FmtResult;
use std::fs::File;
use tracing::{Level, Subscriber};
use tracing_subscriber::{
    EnvFilter,
    fmt::{
        FmtContext,
        format::{FormatEvent, FormatFields, Writer},
    },
    registry::LookupSpan,
};

/// Custom `tracing` event formatter that emits only the log level and message.
///
/// Strips timestamps, targets, and spans to keep CLI output clean.
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
    ) -> FmtResult {
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

/// Initialises the global `tracing` subscriber based on CLI flags.
///
/// If a log file is specified via [`CrawkArgs`], logs are written there at the file
/// verbosity level without ANSI colours. Otherwise, logs go to stderr using
/// [`MinimalFormat`] with the console verbosity level.
///
/// # Errors
///
/// Returns an error if the log file cannot be created.
pub(crate) fn configure_tracing(command: &CrawkArgs) -> anyhow::Result<()> {
    if let Some(log_file_path) = command.log_file() {
        let file = File::create(log_file_path)
            .with_context(|| format!("Failed to create log file '{}'", log_file_path.display()))?;
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

    Ok(())
}
