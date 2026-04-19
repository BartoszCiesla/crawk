use std::fmt::Write;
use std::path::Path;

use comfy_table::presets::ASCII_FULL;
use comfy_table::{ContentArrangement, Table};
use crawk::ModuleInfo;

/// Render module list in plain text format.
///
/// Without `show_source`, outputs one module path per line.
/// With `show_source`, outputs module paths aligned with relative file paths.
pub(crate) fn render_list_plain(
    modules: &[ModuleInfo],
    show_source: bool,
    crate_root: &Path,
) -> String {
    if modules.is_empty() {
        return String::new();
    }

    if !show_source {
        let mut output = String::new();
        for module in modules {
            writeln!(&mut output, "{}", module.path()).ok();
        }
        return output;
    }

    let max_path_len = modules.iter().map(|m| m.path().len()).max().unwrap_or(0);

    let mut output = String::new();
    for module in modules {
        let relative = make_relative(module.source(), crate_root);
        writeln!(
            &mut output,
            "{:<width$}  {}",
            module.path(),
            relative,
            width = max_path_len
        )
        .ok();
    }
    output
}

/// Render module list as a Unicode table.
///
/// Always includes a "Module" column. Includes a "File" column when `show_source` is true.
pub(crate) fn render_list_table(
    modules: &[ModuleInfo],
    show_source: bool,
    crate_root: &Path,
) -> String {
    let mut table = Table::new();
    table
        .load_preset(ASCII_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    if show_source {
        table.set_header(vec!["Module", "File"]);
        for module in modules {
            let relative = make_relative(module.source(), crate_root);
            table.add_row(vec![module.path(), &relative]);
        }
    } else {
        table.set_header(vec!["Module"]);
        for module in modules {
            table.add_row(vec![module.path()]);
        }
    }

    format!("{table}\n")
}

fn make_relative(source: &Path, crate_root: &Path) -> String {
    source
        .strip_prefix(crate_root)
        .unwrap_or(source)
        .to_string_lossy()
        .into_owned()
}
