//! Rule evaluation: a single pass over the dependency graph's edges.

use std::collections::{BTreeSet, HashMap};

use crate::graph::DependencyGraph;

use super::{CheckReport, LayerPos, RuleSet, Violation, ViolationKind};

impl RuleSet {
    /// Build `module -> LayerPos` via longest-prefix match.
    ///
    /// Modules that tie across groups are skipped here; that ambiguity is
    /// reported earlier, during [`RuleSet::load`](super::RuleSet::load).
    pub(crate) fn build_layer_index(
        &self,
        modules: &BTreeSet<String>,
    ) -> HashMap<String, LayerPos> {
        let mut index = HashMap::new();
        for module in modules {
            if let Ok(Some(pos)) = self.resolve_layer(module) {
                index.insert(module.clone(), pos);
            }
        }
        index
    }

    /// Check a single edge against the layer rules, pushing any violation.
    fn check_layers(
        &self,
        source: &str,
        target: &str,
        layer_index: &HashMap<String, LayerPos>,
        apis: &BTreeSet<String>,
        out: &mut Vec<Violation>,
    ) {
        let (Some(src_pos), Some(tgt_pos)) = (layer_index.get(source), layer_index.get(target))
        else {
            return; // at least one endpoint is unassigned
        };
        if src_pos.group != tgt_pos.group {
            return; // independent stacks: no cross-group constraint
        }

        let upward = src_pos.index > tgt_pos.index;
        let same_layer = src_pos.index == tgt_pos.index;
        let violates = upward || (same_layer && self.deny_same_layer);
        if !violates {
            return;
        }

        let group_name = self
            .layers
            .get(src_pos.group)
            .map_or("?", |layer| layer.name.as_str());
        let rule = if upward {
            format!("layer '{group_name}' forbids upward dependency ({source} -> {target})")
        } else {
            format!("layer '{group_name}' forbids same-layer dependency ({source} -> {target})")
        };

        out.push(Violation {
            kind: ViolationKind::Layer,
            source: source.to_owned(),
            target: target.to_owned(),
            rule,
            apis: apis.clone(),
        });
    }
}

/// Evaluate `rules` against `graph`, returning all violations (sorted).
pub(crate) fn evaluate(rules: &RuleSet, graph: &DependencyGraph) -> CheckReport {
    let mut violations = Vec::new();
    let layer_index = rules.build_layer_index(graph.modules());

    for ((source, target), apis) in graph.edges() {
        rules.check_layers(source, target, &layer_index, apis, &mut violations);
    }

    violations.sort();
    CheckReport { violations }
}

#[cfg(test)]
mod tests {
    use super::super::{LayerRule, ModulePattern, RuleSet};
    use std::collections::BTreeSet;

    fn layer(name: &str, order: &[&str]) -> LayerRule {
        LayerRule {
            name: name.to_owned(),
            order: order
                .iter()
                .map(|s| ModulePattern::parse_subtree(s))
                .collect(),
        }
    }

    fn module_set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(ToString::to_string).collect()
    }

    fn app_rules() -> RuleSet {
        RuleSet {
            layers: vec![layer("app", &["cli", "analyzer", "parser", "discover"])],
            strict_layers: false,
            deny_same_layer: false,
        }
    }

    #[test]
    fn downward_dependency_is_allowed() {
        let rules = app_rules();
        let modules = module_set(&["cli", "analyzer"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("cli", "analyzer", &index, &BTreeSet::new(), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn upward_dependency_is_a_violation() {
        let rules = app_rules();
        let modules = module_set(&["cli", "analyzer"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("analyzer", "cli", &index, &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn subtree_inherits_layer() {
        let rules = app_rules();
        // parser::visitor is in the parser subtree (idx 2); cli is idx 0.
        let modules = module_set(&["cli", "parser::visitor"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("parser::visitor", "cli", &index, &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 1, "deep module depending upward must violate");
    }

    #[test]
    fn different_groups_have_no_constraint() {
        let rules = RuleSet {
            layers: vec![
                layer("app", &["cli", "core"]),
                layer("web", &["web::api", "web::repo"]),
            ],
            strict_layers: false,
            deny_same_layer: false,
        };
        let modules = module_set(&["cli", "web::repo"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        // cli (app) -> web::repo (web): cross-group, even though indices differ.
        rules.check_layers("cli", "web::repo", &index, &BTreeSet::new(), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn same_layer_allowed_by_default() {
        let rules = RuleSet {
            layers: vec![layer("app", &["mid"])],
            strict_layers: false,
            deny_same_layer: false,
        };
        let modules = module_set(&["mid::a", "mid::b"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("mid::a", "mid::b", &index, &BTreeSet::new(), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn same_layer_denied_when_flag_set() {
        let rules = RuleSet {
            layers: vec![layer("app", &["mid"])],
            strict_layers: false,
            deny_same_layer: true,
        };
        let modules = module_set(&["mid::a", "mid::b"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("mid::a", "mid::b", &index, &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn unassigned_module_is_skipped() {
        let rules = app_rules();
        let modules = module_set(&["cli"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        // "stray" is not in any layer → no constraint.
        rules.check_layers("stray", "cli", &index, &BTreeSet::new(), &mut out);
        rules.check_layers("cli", "stray", &index, &BTreeSet::new(), &mut out);
        assert!(out.is_empty());
    }
}
