use crawk::TypeReference;
use std::collections::BTreeSet;

pub(crate) mod flat;
pub(crate) mod grouped;

fn truncate_and_dedup<'a>(
    refs: impl IntoIterator<Item = &'a TypeReference>,
    depth: Option<usize>,
) -> Vec<String> {
    refs.into_iter()
        .map(|r| depth.map_or_else(|| r.to_string(), |d| r.truncate_to_depth(d).to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
