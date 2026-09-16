use super::cli::CrawkArgs;
use anyhow::Context;
use owo_colors::OwoColorize;
use std::ffi::OsStr;
use std::fmt::Result as FmtResult;
use std::fs::File;
use std::io::IsTerminal;
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
        if writer.has_ansi_escapes() {
            let colored_level = match level {
                Level::ERROR => level.as_str().red().to_string(),
                Level::WARN => level.as_str().yellow().to_string(),
                Level::INFO => level.as_str().green().to_string(),
                Level::DEBUG => level.as_str().blue().to_string(),
                Level::TRACE => level.as_str().purple().to_string(),
            };
            write!(writer, "{colored_level} ")?;
        } else {
            write!(writer, "{} ", level.as_str())?;
        }
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

/// Whether console log output should use ANSI colours.
///
/// Colours require stderr to be a terminal and `NO_COLOR` to be unset or
/// empty, per <https://no-color.org>.
fn use_ansi_colors(stderr_is_terminal: bool, no_color: Option<&OsStr>) -> bool {
    stderr_is_terminal && no_color.is_none_or(OsStr::is_empty)
}

/// Initialises the global `tracing` subscriber based on CLI flags.
///
/// If a log file is specified via [`CrawkArgs`], logs are written there at the file
/// verbosity level without ANSI colours. Otherwise, logs go to stderr using
/// [`MinimalFormat`] with the console verbosity level; colours are enabled only
/// when stderr is a terminal and the `NO_COLOR` environment variable is unset
/// or empty.
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
        let ansi = use_ansi_colors(
            std::io::stderr().is_terminal(),
            std::env::var_os("NO_COLOR").as_deref(),
        );
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .with_ansi(ansi)
            .event_format(MinimalFormat)
            .init();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::use_ansi_colors;
    use std::ffi::OsStr;

    #[test]
    fn colors_on_terminal_without_no_color() {
        assert!(use_ansi_colors(true, None));
    }

    #[test]
    fn no_colors_when_not_a_terminal() {
        assert!(!use_ansi_colors(false, None));
        assert!(!use_ansi_colors(false, Some(OsStr::new("1"))));
    }

    #[test]
    fn no_color_env_disables_colors() {
        assert!(!use_ansi_colors(true, Some(OsStr::new("1"))));
        // Spec: any non-empty value disables colours, even "0".
        assert!(!use_ansi_colors(true, Some(OsStr::new("0"))));
    }

    #[test]
    fn empty_no_color_is_ignored() {
        assert!(use_ansi_colors(true, Some(OsStr::new(""))));
    }
}
