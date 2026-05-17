pub(crate) mod cycles;
pub(crate) mod deps_cmd;
pub(crate) mod list_cmd;
pub(crate) mod orphans;
pub(crate) mod use_cmd;
pub(crate) mod why_cmd;

use std::collections::BTreeSet;

/// Format an API annotation suffix for a single edge.
///
/// Returns ` [Symbol1, Symbol2]` when APIs are present, empty string otherwise.
pub(super) fn format_api_suffix(apis: &BTreeSet<String>) -> String {
    if apis.is_empty() {
        return String::new();
    }
    let items: Vec<&str> = apis.iter().map(String::as_str).collect();
    format!(" [{}]", items.join(", "))
}
