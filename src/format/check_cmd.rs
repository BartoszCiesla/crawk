//! Rendering for the `check` command: plain violation report.

use std::fmt::Write as _;

use crawk::CheckReport;

use super::format_api_suffix;

/// Render a violation report as a plain, CI-friendly text block.
///
/// Header line with the count, then one line per violation:
/// `KIND   source -> target [apis]   (rule: …)`. Returns an empty string when
/// the report is clean.
pub(crate) fn render_plain(report: &CheckReport) -> String {
    if report.violations.is_empty() {
        return String::new();
    }

    let count = report.violations.len();
    let noun = if count == 1 {
        "violation"
    } else {
        "violations"
    };

    let mut out = String::new();
    let _ = writeln!(out, "crawk check: {count} {noun}");
    out.push('\n');
    for violation in &report.violations {
        let _ = writeln!(
            out,
            "  {:<6} {} -> {}{}   (rule: {})",
            violation.kind,
            violation.source,
            violation.target,
            format_api_suffix(&violation.apis),
            violation.rule,
        );
    }
    out
}
