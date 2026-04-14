use crawk::AnalysisResult;

use super::truncate_and_dedup;

/// Renders dependencies as a flat, alphabetically sorted list.
///
/// Each dependency occupies one line. Duplicates produced by depth truncation are
/// removed. Returns an empty string when there are no dependencies.
pub(crate) fn render_flat(result: &AnalysisResult, depth: Option<usize>) -> String {
    let mut output = truncate_and_dedup(result.dependencies().values().flatten(), depth).join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}
