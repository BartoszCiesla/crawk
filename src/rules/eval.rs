//! Rule evaluation: a single pass over the dependency graph's edges.

use std::collections::{BTreeSet, HashMap};

use crate::graph::DependencyGraph;

use super::{CheckReport, LayerPos, RuleSet, Violation, ViolationKind};

impl RuleSet {
    /// Build `module -> [LayerPos]` via per-group longest-prefix match.
    ///
    /// A module may belong to several overlapping groups, so each maps to a
    /// list of positions (one per covering group). Uncovered modules are
    /// omitted.
    pub(crate) fn build_layer_index(
        &self,
        modules: &BTreeSet<String>,
    ) -> HashMap<String, Vec<LayerPos>> {
        let mut index = HashMap::new();
        for module in modules {
            let positions = self.memberships(module);
            if !positions.is_empty() {
                index.insert(module.clone(), positions);
            }
        }
        index
    }

    /// Check a single edge against the deny rules, pushing any violation.
    ///
    /// Each matching rule contributes its own violation, so an edge banned by
    /// several rules is reported once per rule (consistent with overlapping
    /// layer groups).
    fn check_deny(
        &self,
        source: &str,
        target: &str,
        apis: &BTreeSet<String>,
        out: &mut Vec<Violation>,
    ) {
        for rule in &self.deny {
            if rule.from.matches(source) && rule.to.matches(target) {
                out.push(Violation {
                    kind: ViolationKind::Deny,
                    source: source.to_owned(),
                    target: target.to_owned(),
                    rule: rule.display(),
                    apis: apis.clone(),
                });
            }
        }
    }

    /// Check a single edge against the layer rules, pushing any violation.
    ///
    /// Each group containing **both** endpoints is checked independently, so an
    /// edge forbidden by several overlapping groups produces one violation per
    /// group (the rule message names the offending group).
    fn check_layers(
        &self,
        source: &str,
        target: &str,
        layer_index: &HashMap<String, Vec<LayerPos>>,
        apis: &BTreeSet<String>,
        out: &mut Vec<Violation>,
    ) {
        let (Some(src_positions), Some(tgt_positions)) =
            (layer_index.get(source), layer_index.get(target))
        else {
            return; // at least one endpoint is unassigned
        };

        for src_pos in src_positions {
            // The edge is only constrained where both endpoints share a group.
            let Some(tgt_pos) = tgt_positions.iter().find(|t| t.group == src_pos.group) else {
                continue; // independent stacks: no cross-group constraint
            };

            // Fetch the covering group once; its `deny_same_layer` is per-group.
            let layer = self.layers.get(src_pos.group);
            let deny_same = layer.is_some_and(|l| l.deny_same_layer);
            let upward = src_pos.index > tgt_pos.index;
            let same_layer = src_pos.index == tgt_pos.index;
            let violates = upward || (same_layer && deny_same);
            if !violates {
                continue;
            }

            let group_name = layer.map_or("?", |layer| layer.name.as_str());
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
}

/// Evaluate `rules` against `graph`, returning all violations (sorted).
pub(crate) fn evaluate(rules: &RuleSet, graph: &DependencyGraph) -> CheckReport {
    let mut violations = Vec::new();
    let layer_index = rules.build_layer_index(graph.modules());

    for ((source, target), apis) in graph.edges() {
        rules.check_deny(source, target, apis, &mut violations);
        rules.check_layers(source, target, &layer_index, apis, &mut violations);
    }

    violations.sort();
    CheckReport { violations }
}

#[cfg(test)]
mod tests {
    use super::super::{DenyRule, LayerRule, ModulePattern, RuleSet, ViolationKind};
    use std::collections::BTreeSet;

    fn layer(name: &str, order: &[&str]) -> LayerRule {
        layer_with_deny(name, order, false)
    }

    fn layer_deny(name: &str, order: &[&str]) -> LayerRule {
        layer_with_deny(name, order, true)
    }

    fn layer_with_deny(name: &str, order: &[&str], deny_same_layer: bool) -> LayerRule {
        LayerRule {
            name: name.to_owned(),
            order: order
                .iter()
                .map(|s| ModulePattern::parse_subtree(s))
                .collect(),
            deny_same_layer,
        }
    }

    fn module_set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(ToString::to_string).collect()
    }

    fn app_rules() -> RuleSet {
        RuleSet {
            layers: vec![layer("app", &["cli", "analyzer", "parser", "discover"])],
            deny: Vec::new(),
            strict_layers: false,
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
            deny: Vec::new(),
            strict_layers: false,
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
            deny: Vec::new(),
            strict_layers: false,
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
            layers: vec![layer_deny("app", &["mid"])],
            deny: Vec::new(),
            strict_layers: false,
        };
        let modules = module_set(&["mid::a", "mid::b"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("mid::a", "mid::b", &index, &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn deny_same_layer_is_per_group() {
        // `strict` denies same-layer deps; `lax` (same modules) does not. The
        // edge a -> b is same-layer in both, so only `strict` yields a violation.
        let rules = RuleSet {
            layers: vec![layer_deny("strict", &["mid"]), layer("lax", &["mid"])],
            deny: Vec::new(),
            strict_layers: false,
        };
        let modules = module_set(&["mid::a", "mid::b"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("mid::a", "mid::b", &index, &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 1, "only the deny group contributes a violation");
        assert!(out[0].rule.contains("'strict'"), "{}", out[0].rule);
    }

    #[test]
    fn overlapping_groups_each_yield_a_violation() {
        // `mid` and `top` belong to both groups, each reversing their order, so
        // the edge mid -> top is upward in both → one violation per group.
        let rules = RuleSet {
            layers: vec![
                layer("left", &["top", "mid"]),
                layer("right", &["top", "mid"]),
            ],
            deny: Vec::new(),
            strict_layers: false,
        };
        let modules = module_set(&["top", "mid"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("mid", "top", &index, &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 2, "one violation per overlapping group");
    }

    #[test]
    fn overlapping_groups_clean_when_each_satisfied() {
        // `shared` is index 0 in both groups; each edge is downward in its own
        // group, so overlap produces no violation.
        let rules = RuleSet {
            layers: vec![
                layer("a", &["shared", "low_a"]),
                layer("b", &["shared", "low_b"]),
            ],
            deny: Vec::new(),
            strict_layers: false,
        };
        let modules = module_set(&["shared", "low_a", "low_b"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("shared", "low_a", &index, &BTreeSet::new(), &mut out);
        rules.check_layers("shared", "low_b", &index, &BTreeSet::new(), &mut out);
        assert!(out.is_empty());
    }

    fn deny(from: &str, to: &str) -> DenyRule {
        DenyRule {
            from: ModulePattern::parse(from),
            to: ModulePattern::parse(to),
        }
    }

    fn deny_rules(rules: Vec<DenyRule>) -> RuleSet {
        RuleSet {
            layers: Vec::new(),
            deny: rules,
            strict_layers: false,
        }
    }

    #[test]
    fn deny_flags_matching_edge() {
        let rules = deny_rules(vec![deny("cli", "web::*")]);
        let mut out = Vec::new();
        rules.check_deny("cli", "web::repo", &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, ViolationKind::Deny);
        assert_eq!(out[0].rule, "deny cli -> web::*");
    }

    #[test]
    fn deny_ignores_non_matching_edge() {
        let rules = deny_rules(vec![deny("cli", "web::*")]);
        let mut out = Vec::new();
        // cli::args is not `cli` (no explicit ::*); webs is not under web::*.
        rules.check_deny("cli::args", "web::repo", &BTreeSet::new(), &mut out);
        rules.check_deny("cli", "webs", &BTreeSet::new(), &mut out);
        rules.check_deny("analyzer", "web::repo", &BTreeSet::new(), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn deny_subtree_matches_base_and_descendants() {
        let rules = deny_rules(vec![deny("format::*", "discover")]);
        let mut out = Vec::new();
        rules.check_deny("format", "discover", &BTreeSet::new(), &mut out);
        rules.check_deny("format::deps_cmd", "discover", &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn edge_banned_by_several_rules_reports_each() {
        let rules = deny_rules(vec![deny("cli", "web::*"), deny("*", "web::repo")]);
        let mut out = Vec::new();
        rules.check_deny("cli", "web::repo", &BTreeSet::new(), &mut out);
        assert_eq!(out.len(), 2, "one violation per matching rule");
    }

    #[test]
    fn deny_sorts_before_layer_violations() {
        // The same edge breaks a deny rule and a layer order; the derived Ord on
        // Violation compares `kind` first, so DENY rows lead the report.
        let rules = RuleSet {
            layers: vec![layer("app", &["low", "high"])],
            deny: vec![deny("high", "low")],
            strict_layers: false,
        };
        let modules = module_set(&["high", "low"]);
        let index = rules.build_layer_index(&modules);
        let mut out = Vec::new();
        rules.check_layers("high", "low", &index, &BTreeSet::new(), &mut out);
        rules.check_deny("high", "low", &BTreeSet::new(), &mut out);
        out.sort();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].kind, ViolationKind::Deny);
        assert_eq!(out[1].kind, ViolationKind::Layer);
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
