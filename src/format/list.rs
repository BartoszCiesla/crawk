use std::fmt::Write;
use std::path::Path;

use comfy_table::presets::ASCII_FULL;
use comfy_table::{ContentArrangement, Table};
use crawk::ModuleInfo;

/// Render module list in plain text format.
///
/// Columns (left to right): module path, visibility (if `show_visibility`), file (if `show_source`).
pub(crate) fn render_list_plain(
    modules: &[ModuleInfo],
    show_source: bool,
    show_visibility: bool,
    crate_root: &Path,
) -> String {
    if modules.is_empty() {
        return String::new();
    }

    let mod_width = if show_source || show_visibility {
        modules.iter().map(|m| m.path().len()).max().unwrap_or(0)
    } else {
        0
    };
    let vis_width = if show_visibility && show_source {
        modules
            .iter()
            .map(|m| m.visibility().to_string().len())
            .max()
            .unwrap_or(0)
    } else {
        0
    };

    let mut output = String::new();
    for module in modules {
        let path = module.path();
        let vis = module.visibility().to_string();

        match (show_visibility, show_source) {
            (false, false) => writeln!(&mut output, "{path}").ok(),
            (false, true) => {
                let source = make_relative(module.source(), crate_root);
                writeln!(&mut output, "{path:<mod_width$}  {source}").ok()
            }
            (true, false) => writeln!(&mut output, "{path:<mod_width$}  {vis}").ok(),
            (true, true) => {
                let source = make_relative(module.source(), crate_root);
                writeln!(
                    &mut output,
                    "{path:<mod_width$}  {vis:<vis_width$}  {source}"
                )
                .ok()
            }
        };
    }
    output
}

/// Render module list as a Unicode table.
///
/// Columns (left to right): Module, Visibility (if `show_visibility`), File (if `show_source`).
pub(crate) fn render_list_table(
    modules: &[ModuleInfo],
    show_source: bool,
    show_visibility: bool,
    crate_root: &Path,
) -> String {
    let mut table = Table::new();
    table
        .load_preset(ASCII_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    match (show_visibility, show_source) {
        (false, false) => {
            table.set_header(vec!["Module"]);
            for module in modules {
                table.add_row(vec![module.path()]);
            }
        }
        (false, true) => {
            table.set_header(vec!["Module", "File"]);
            for module in modules {
                let relative = make_relative(module.source(), crate_root);
                table.add_row(vec![module.path(), &relative]);
            }
        }
        (true, false) => {
            table.set_header(vec!["Module", "Visibility"]);
            for module in modules {
                let vis = module.visibility().to_string();
                table.add_row(vec![module.path(), &vis]);
            }
        }
        (true, true) => {
            table.set_header(vec!["Module", "Visibility", "File"]);
            for module in modules {
                let vis = module.visibility().to_string();
                let relative = make_relative(module.source(), crate_root);
                table.add_row(vec![module.path(), &vis, &relative]);
            }
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
