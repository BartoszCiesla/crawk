use std::fmt::Write;
use std::path::Path;

use comfy_table::presets::ASCII_FULL;
use comfy_table::{ContentArrangement, Table};
use crawk::ModuleInfo;

/// Display options controlling which columns appear in the list output.
pub(crate) struct ListDisplayOptions {
    /// Show source file paths alongside module names.
    pub show_source: bool,
    /// Show visibility column.
    pub show_visibility: bool,
    /// Show target tag column (e.g. `[lib]`, `[bin:crawk]`).
    pub multi_target: bool,
}

/// Render module list in plain text format.
///
/// Columns (left to right): target tag (if `multi_target`), module path,
/// visibility (if `show_visibility`), file (if `show_source`).
#[must_use]
pub(crate) fn render_list_plain(
    modules: &[ModuleInfo],
    opts: &ListDisplayOptions,
    crate_root: &Path,
) -> String {
    if modules.is_empty() {
        return String::new();
    }

    let needs_columns = opts.show_source || opts.show_visibility || opts.multi_target;

    let tag_width = if opts.multi_target {
        modules
            .iter()
            .map(|m| format_target_tag(m).len())
            .max()
            .unwrap_or(0)
    } else {
        0
    };
    let mod_width = if needs_columns {
        modules.iter().map(|m| m.path().len()).max().unwrap_or(0)
    } else {
        0
    };
    let vis_width = if opts.show_visibility && (opts.show_source || opts.multi_target) {
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

        if opts.multi_target {
            let tag = format_target_tag(module);
            write!(&mut output, "{tag:<tag_width$} ").ok();
        }

        match (opts.show_visibility, opts.show_source) {
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
/// Columns (left to right): Target (if `multi_target`), Module,
/// Visibility (if `show_visibility`), File (if `show_source`).
#[must_use]
pub(crate) fn render_list_table(
    modules: &[ModuleInfo],
    opts: &ListDisplayOptions,
    crate_root: &Path,
) -> String {
    let mut table = Table::new();
    table
        .load_preset(ASCII_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    // Build header
    let mut header: Vec<&str> = Vec::new();
    if opts.multi_target {
        header.push("Target");
    }
    header.push("Module");
    if opts.show_visibility {
        header.push("Visibility");
    }
    if opts.show_source {
        header.push("File");
    }
    table.set_header(header);

    // Build rows
    for module in modules {
        let mut row: Vec<String> = Vec::new();
        if opts.multi_target {
            row.push(module.target().to_string());
        }
        row.push(module.path().to_owned());
        if opts.show_visibility {
            row.push(module.visibility().to_string());
        }
        if opts.show_source {
            row.push(make_relative(module.source(), crate_root));
        }
        table.add_row(row);
    }

    format!("{table}\n")
}

/// Format the target tag for plain output, e.g. `[lib]`, `[bin:crawk]`.
fn format_target_tag(module: &ModuleInfo) -> String {
    format!("[{}]", module.target())
}

fn make_relative(source: &Path, crate_root: &Path) -> String {
    source
        .strip_prefix(crate_root)
        .unwrap_or(source)
        .to_string_lossy()
        .into_owned()
}
