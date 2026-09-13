//! Rendering for the `check` command: plain violation report.

use std::fmt::Write as _;

use crawk::CheckReport;

use super::format_api_suffix;

/// Render a violation report as a plain, CI-friendly text block.
///
/// Header line with the count, then one line per violation:
/// `KIND source -> target [apis]   (rule: …)`. The kind column is padded only
/// as far as this report needs: a single-kind report keeps one plain space,
/// a mixed report aligns every row on its widest kind. Returns an empty
/// string when the report is clean.
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

    let width = report
        .violations
        .iter()
        .map(|violation| violation.kind.to_string().len())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    let _ = writeln!(out, "crawk check: {count} {noun}");
    out.push('\n');
    for violation in &report.violations {
        let _ = writeln!(
            out,
            "  {:<width$} {} -> {}{}   (rule: {})",
            violation.kind,
            violation.source,
            violation.target,
            format_api_suffix(&violation.apis),
            violation.rule,
        );
    }
    out
}
