use crawk::AnalysisResult;

use super::truncate_and_dedup;

pub(crate) fn render_flat(result: &AnalysisResult, depth: Option<usize>) -> String {
    let mut output = truncate_and_dedup(result.dependencies().values().flatten(), depth).join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}
