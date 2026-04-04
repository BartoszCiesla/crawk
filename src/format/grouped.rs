use crawk::AnalysisResult;

use super::truncate_and_dedup;

pub(crate) fn render_grouped(result: &AnalysisResult, depth: Option<usize>) -> String {
    let dependencies = result.dependencies();
    let mut modules = dependencies.keys().cloned().collect::<Vec<_>>();
    modules.sort();

    let mut out = String::new();
    for module in modules {
        let display_name = if module.is_empty() {
            result.module_path()
        } else {
            module.as_str()
        };
        out.push_str(display_name);
        out.push('\n');
        for reference in truncate_and_dedup(&dependencies[&module], depth) {
            out.push_str(" - ");
            out.push_str(&reference);
            out.push('\n');
        }
    }
    out
}
