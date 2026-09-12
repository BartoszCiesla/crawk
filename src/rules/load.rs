//! Config-file resolution, deserialization, and validation.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::Spanned;
use tracing::warn;

use crate::error::{AnalysisError, Result};
use crate::graph::Cycle;

use super::{
    AllowedCycle, DenyRule, InitOutcome, LayerRule, ModulePattern, RuleSet, is_parent_child_cycle,
};

/// Preferred config file name (searched first).
const CONFIG_FILE: &str = "crawk.toml";
/// Hidden config file name, searched as a fallback.
const CONFIG_FILE_HIDDEN: &str = ".crawk.toml";

/// Serde shape of the whole config file: a single `[check]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    check: RawCheck,
}

/// Serde shape of the `[check]` table.
///
/// `deny_same_layer` here is the crate-wide *default*; each `[[check.layers]]`
/// group may override it (see [`RawLayer`]).
// A deserialization shape, not an API: the bools mirror independent TOML keys,
// so folding them into enums would only distort the config surface.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct RawCheck {
    layers: Vec<RawLayer>,
    deny: Vec<RawDeny>,
    allow_cycle: Vec<RawAllowCycle>,
    strict_layers: bool,
    deny_same_layer: bool,
    deny_cycles: bool,
    deny_parent_child_cycles: bool,
}

/// Serde shape of one `[[check.allow-cycle]]` entry: a loop that predates the
/// rule and is tolerated until it is untangled.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAllowCycle {
    modules: Vec<String>,
    #[serde(default)]
    reason: Option<String>,
}

/// Serde shape of one `[[check.deny]]` rule. Subtree matching requires an
/// explicit `::*` suffix on the pattern.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeny {
    from: String,
    to: String,
}

/// Serde shape of one `[[check.layers]]` group.
///
/// `name` is [`Spanned`] so a duplicate group name can be
/// reported with the source line it occurs on. `deny_same_layer` is optional:
/// `None` inherits the `[check]`-level default.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct RawLayer {
    name: Spanned<String>,
    order: Vec<String>,
    #[serde(default)]
    deny_same_layer: Option<bool>,
}

/// Resolve which rule-config file to use.
///
/// With an explicit path, that file is used verbatim (error if missing). Without
/// one, the crate root is searched for `crawk.toml` then `.crawk.toml`; the plain
/// name wins when both exist (with a warning). Missing config is an operational
/// error so a typo'd filename fails CI rather than silently passing.
pub(crate) fn resolve_config_path(crate_root: &Path, explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(AnalysisError::RuleConfigError {
            path: path.to_path_buf(),
            reason: "config file does not exist".to_owned(),
        });
    }

    let plain = crate_root.join(CONFIG_FILE);
    let hidden = crate_root.join(CONFIG_FILE_HIDDEN);
    match (plain.is_file(), hidden.is_file()) {
        (true, true) => {
            warn!("both {CONFIG_FILE} and {CONFIG_FILE_HIDDEN} found, using {CONFIG_FILE}");
            Ok(plain)
        }
        (true, false) => Ok(plain),
        (false, true) => Ok(hidden),
        (false, false) => Err(AnalysisError::RuleConfigError {
            path: crate_root.to_path_buf(),
            reason: format!(
                "no {CONFIG_FILE} or {CONFIG_FILE_HIDDEN} found in {}; \
                 run `crawk check --init` to generate a starter config",
                crate_root.display()
            ),
        }),
    }
}

/// Quote and join module names into a TOML array body.
fn toml_list<'a>(names: impl Iterator<Item = &'a str>) -> String {
    names
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The loops worth writing into a starter config, sorted for a stable file.
///
/// Containment loops are left out: `deny-cycles` never reports them, so an entry
/// for one would be noise the reader has to disprove.
fn freezable_cycles(cycles: &[Cycle]) -> Vec<&BTreeSet<String>> {
    let mut frozen: Vec<&BTreeSet<String>> = cycles
        .iter()
        .map(|cycle| &cycle.modules)
        .filter(|modules| !is_parent_child_cycle(modules))
        .collect();
    frozen.sort();
    frozen
}

/// Render a starter `crawk.toml` body from the crate's modules and cycles.
///
/// Emits a single `[[check.layers]]` group named after the crate, listing its
/// top-level modules alphabetically, with a comment telling the user to order
/// them. crawk deliberately does not guess the hierarchy — layer ordering
/// encodes intent the source cannot reveal.
///
/// Cycles are the opposite case: the source *does* reveal them, so the scaffold
/// turns `deny-cycles` on and freezes every existing loop as an
/// `[[check.allow-cycle]]` entry. The rule therefore starts green and ratchets
/// from day one instead of drowning a new adopter in pre-existing tangles.
pub(crate) fn scaffold(crate_name: &str, modules: &BTreeSet<String>, cycles: &[Cycle]) -> String {
    // Collapse to top-level segments; the BTreeSet dedups and sorts them.
    let top: BTreeSet<&str> = modules
        .iter()
        .filter_map(|m| m.split("::").next())
        .collect();
    let order = toml_list(top.into_iter());
    // The `[check]` keys must precede the first `[[check.layers]]`: in TOML,
    // bare keys after an array-of-tables header belong to that table.
    let mut out = format!(
        "# Generated by `crawk check --init`. Reorder `order` so the highest\n\
         # layer comes first: a lower layer must not depend on a higher one.\n\
         # Split into multiple groups as needed; a module may appear in several,\n\
         # each checked independently. See `crawk check\n\
         # --help` for the full `layers` semantics.\n\
         \n\
         [check]\n\
         # No dependency loops. A parent tangled with its own submodules is\n\
         # containment, not a tangle, and stays exempt unless you also set\n\
         # `deny-parent-child-cycles = true`.\n\
         deny-cycles = true\n\
         \n\
         [[check.layers]]\n\
         name = \"{crate_name}\"\n\
         order = [{order}]\n"
    );

    let frozen = freezable_cycles(cycles);
    if !frozen.is_empty() {
        out.push_str(
            "\n# Loops this crate already has, frozen so `deny-cycles` starts green.\n\
             # Delete an entry once its loop is untangled; a module joining a frozen\n\
             # loop falls outside the entry and is reported.\n",
        );
        for loop_modules in frozen {
            let list = toml_list(loop_modules.iter().map(String::as_str));
            let _ = write!(out, "\n[[check.allow-cycle]]\nmodules = [{list}]\n");
        }
    }
    out
}

/// Write a starter rule config, reporting where it landed and how many existing
/// loops it froze.
///
/// With an explicit path (`--config`/`-c`), writes there. Otherwise, writes
/// `crawk.toml` in `crate_root`. Either way it refuses (operational error) when
/// the target — or, for the default case, any discovered `crawk.toml` /
/// `.crawk.toml` — already exists, so `--init` never clobbers a real config.
///
/// # Errors
///
/// [`AnalysisError::RuleConfigError`] when a config already exists or the file
/// cannot be written.
pub(crate) fn scaffold_config(
    crate_root: &Path,
    explicit: Option<&Path>,
    crate_name: &str,
    modules: &BTreeSet<String>,
    cycles: &[Cycle],
) -> Result<InitOutcome> {
    let path = if let Some(target) = explicit {
        if target.exists() {
            return Err(AnalysisError::RuleConfigError {
                path: target.to_path_buf(),
                reason: "config already exists; edit it directly or remove it first".to_owned(),
            });
        }
        target.to_path_buf()
    } else {
        if let Ok(existing) = resolve_config_path(crate_root, None) {
            return Err(AnalysisError::RuleConfigError {
                path: existing,
                reason: "config already exists; edit it directly or remove it first".to_owned(),
            });
        }
        crate_root.join(CONFIG_FILE)
    };
    std::fs::write(&path, scaffold(crate_name, modules, cycles)).map_err(|e| {
        AnalysisError::RuleConfigError {
            path: path.clone(),
            reason: e.to_string(),
        }
    })?;
    Ok(InitOutcome {
        path,
        frozen_cycles: freezable_cycles(cycles).len(),
    })
}

/// 1-based line number of byte `offset` within `text`.
fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.get(..end)
        .unwrap_or(text)
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// Reject duplicate `[[check.layers]]` group names, pinning the duplicate to its
/// source line via the [`Spanned`] name.
fn check_unique_names(path: &Path, text: &str, layers: &[RawLayer]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for layer in layers {
        if !seen.insert(layer.name.get_ref().as_str()) {
            let line = line_of(text, layer.name.span().start);
            return Err(AnalysisError::RuleConfigError {
                path: path.to_path_buf(),
                reason: format!(
                    "duplicate layer group name '{}' (line {line})",
                    layer.name.get_ref()
                ),
            });
        }
    }
    Ok(())
}

impl RuleSet {
    /// Read, parse, and validate a rule-config file against the crate's modules.
    ///
    /// # Errors
    ///
    /// - [`AnalysisError::RuleConfigError`] — file unreadable, malformed TOML,
    ///   a duplicate or one-module `allow-cycle` entry, or (under
    ///   `strict-layers`) an uncovered module.
    /// - [`AnalysisError::UnknownRuleModule`] — a rule names a module that does
    ///   not exist in the crate.
    pub(crate) fn load(path: &Path, modules: &BTreeSet<String>) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| AnalysisError::RuleConfigError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        let raw: RawConfig = toml::from_str(&text).map_err(|e| AnalysisError::RuleConfigError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        // Group names must be unique (violations are reported per group and name
        // it). Checked here, where the spans into `text` are still available, so
        // the duplicate can be pinned to its source line.
        check_unique_names(path, &text, &raw.check.layers)?;
        let rules = Self::from_raw(raw.check);
        rules.validate(path, modules)?;
        if !rules.deny_cycles && !rules.allow_cycles.is_empty() {
            warn!("allow-cycle entries have no effect while deny-cycles is false");
        }
        Ok(rules)
    }

    /// Convert the deserialized shape into the validated in-memory form.
    fn from_raw(raw: RawCheck) -> Self {
        // The `[check]`-level flag is the default each group inherits unless it
        // sets its own `deny-same-layer`.
        let default_deny = raw.deny_same_layer;
        let layers = raw
            .layers
            .into_iter()
            .map(|raw_layer| LayerRule {
                name: raw_layer.name.into_inner(),
                order: raw_layer
                    .order
                    .iter()
                    .map(|entry| ModulePattern::parse_subtree(entry))
                    .collect(),
                deny_same_layer: raw_layer.deny_same_layer.unwrap_or(default_deny),
            })
            .collect();
        let deny = raw
            .deny
            .into_iter()
            .map(|raw_deny| DenyRule {
                from: ModulePattern::parse(&raw_deny.from),
                to: ModulePattern::parse(&raw_deny.to),
            })
            .collect();
        let allow_cycles = raw
            .allow_cycle
            .into_iter()
            .map(|raw_entry| AllowedCycle {
                modules: raw_entry.modules.into_iter().collect(),
                reason: raw_entry.reason,
            })
            .collect();
        Self {
            layers,
            deny,
            allow_cycles,
            strict_layers: raw.strict_layers,
            deny_cycles: raw.deny_cycles,
            deny_parent_child_cycles: raw.deny_parent_child_cycles,
        }
    }

    /// Validate rules against the crate's known modules.
    fn validate(&self, path: &Path, modules: &BTreeSet<String>) -> Result<()> {
        // 1. Every layer pattern must reference a real module (catch typos).
        for layer in &self.layers {
            for pattern in &layer.order {
                if !pattern.references_known(modules) {
                    return Err(AnalysisError::UnknownRuleModule {
                        module: pattern.display(),
                        rule: format!("layers '{}'", layer.name),
                    });
                }
            }
        }
        // 2. Both patterns of every deny rule must reference a real module.
        for rule in &self.deny {
            for pattern in [&rule.from, &rule.to] {
                if !pattern.references_known(modules) {
                    return Err(AnalysisError::UnknownRuleModule {
                        module: pattern.pattern_display(),
                        rule: rule.display(),
                    });
                }
            }
        }
        // 3. Allowlist entries name exact modules and must be able to match at
        //    all — a typo or a one-module entry would silently never fire.
        let mut seen: BTreeSet<&BTreeSet<String>> = BTreeSet::new();
        for entry in &self.allow_cycles {
            if entry.modules.len() < 2 {
                return Err(AnalysisError::RuleConfigError {
                    path: path.to_path_buf(),
                    reason: format!("{}: a cycle needs at least two modules", entry.display()),
                });
            }
            for module in &entry.modules {
                if !modules.contains(module) {
                    return Err(AnalysisError::UnknownRuleModule {
                        module: module.clone(),
                        rule: entry.display(),
                    });
                }
            }
            if !seen.insert(&entry.modules) {
                return Err(AnalysisError::RuleConfigError {
                    path: path.to_path_buf(),
                    reason: format!("duplicate {}", entry.display()),
                });
            }
        }
        // 4. Under strict mode, every module must be covered by some group.
        //    Groups may overlap, so coverage just means at least one membership.
        if self.strict_layers {
            for module in modules {
                if self.memberships(module).is_empty() {
                    return Err(AnalysisError::RuleConfigError {
                        path: path.to_path_buf(),
                        reason: format!(
                            "strict-layers: module '{module}' is not assigned to any layer"
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::AnnotatedEdges;
    use std::io::Write;

    fn write_config(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).expect("create config");
        file.write_all(body.as_bytes()).expect("write config");
        path
    }

    fn module_set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn explicit_path_used_verbatim() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(dir.path(), "custom.toml", "[check]\n");
        let resolved = resolve_config_path(dir.path(), Some(&cfg)).expect("resolve");
        assert_eq!(resolved, cfg);
    }

    #[test]
    fn explicit_missing_path_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope.toml");
        assert!(resolve_config_path(dir.path(), Some(&missing)).is_err());
    }

    #[test]
    fn plain_name_wins_over_hidden() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_config(dir.path(), "crawk.toml", "[check]\n");
        write_config(dir.path(), ".crawk.toml", "[check]\n");
        let resolved = resolve_config_path(dir.path(), None).expect("resolve");
        assert_eq!(resolved, dir.path().join("crawk.toml"));
    }

    #[test]
    fn hidden_name_used_when_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_config(dir.path(), ".crawk.toml", "[check]\n");
        let resolved = resolve_config_path(dir.path(), None).expect("resolve");
        assert_eq!(resolved, dir.path().join(".crawk.toml"));
    }

    #[test]
    fn no_config_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(resolve_config_path(dir.path(), None).is_err());
    }

    #[test]
    fn unknown_module_in_layer_is_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.layers]]\nname = \"app\"\norder = [\"cli\", \"typo_mod\"]\n",
        );
        let modules = module_set(&["cli", "analyzer"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should reject unknown module");
        assert!(matches!(err, AnalysisError::UnknownRuleModule { .. }));
    }

    #[test]
    fn overlapping_groups_are_allowed() {
        let dir = tempfile::tempdir().expect("tempdir");
        // `parser` belongs to both groups; overlap is no longer a config error.
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.layers]]\nname = \"a\"\norder = [\"cli\", \"parser\"]\n\
             [[check.layers]]\nname = \"b\"\norder = [\"parser\", \"discover\"]\n",
        );
        let modules = module_set(&["cli", "parser", "discover"]);
        assert!(RuleSet::load(&cfg, &modules).is_ok());
    }

    #[test]
    fn duplicate_group_names_are_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.layers]]\nname = \"app\"\norder = [\"cli\"]\n\
             [[check.layers]]\nname = \"app\"\norder = [\"analyzer\"]\n",
        );
        let modules = module_set(&["cli", "analyzer"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should reject duplicate names");
        assert!(matches!(&err, AnalysisError::RuleConfigError { .. }));
        if let AnalysisError::RuleConfigError { reason, .. } = err {
            // The duplicate "app" sits on line 5 of the config; the message pins it.
            assert!(
                reason.contains("duplicate layer group name 'app'"),
                "{reason}"
            );
            assert!(reason.contains("line 5"), "{reason}");
        }
    }

    #[test]
    fn strict_layers_requires_full_coverage() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\nstrict-layers = true\n\
             [[check.layers]]\nname = \"app\"\norder = [\"cli\"]\n",
        );
        let modules = module_set(&["cli", "uncovered"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should require coverage");
        assert!(matches!(err, AnalysisError::RuleConfigError { .. }));
    }

    #[test]
    fn unknown_top_level_key_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(dir.path(), "crawk.toml", "[check]\nlayerz = []\n");
        let modules = module_set(&["cli"]);
        assert!(RuleSet::load(&cfg, &modules).is_err());
    }

    #[test]
    fn scaffold_lists_top_level_modules() {
        let modules = module_set(&["cli", "analyzer", "web", "web::api", "web::repo"]);
        let body = scaffold("my_crate", &modules, &[]);
        assert!(body.contains("[[check.layers]]"));
        // Group is named after the crate.
        assert!(body.contains("name = \"my_crate\""));
        // Top-level segments only, deduplicated and alphabetical.
        assert!(body.contains(r#"order = ["analyzer", "cli", "web"]"#));
        assert!(!body.contains("web::api"));
    }

    fn cycle_of(modules: &[&str]) -> Cycle {
        Cycle::new(module_set(modules), AnnotatedEdges::new())
    }

    #[test]
    fn scaffold_enables_the_cycle_rule() {
        let body = scaffold("my_crate", &module_set(&["cli"]), &[]);
        // The `[check]` table must precede the first `[[check.layers]]`, or TOML
        // would attach the key to the layer group instead.
        let check_at = body.find("[check]").expect("[check] table");
        let layers_at = body.find("[[check.layers]]").expect("layers group");
        assert!(check_at < layers_at, "{body}");
        assert!(body.contains("deny-cycles = true"), "{body}");
        // Nothing to freeze in an acyclic crate.
        assert!(!body.contains("[[check.allow-cycle]]"), "{body}");
    }

    #[test]
    fn scaffold_freezes_existing_cycles() {
        let modules = module_set(&["alpha", "beta", "gamma", "delta", "epsilon"]);
        let cycles = [
            cycle_of(&["delta", "epsilon"]),
            cycle_of(&["alpha", "beta", "gamma"]),
        ];
        let body = scaffold("my_crate", &modules, &cycles);
        assert!(
            body.contains(r#"modules = ["alpha", "beta", "gamma"]"#),
            "{body}"
        );
        assert!(body.contains(r#"modules = ["delta", "epsilon"]"#), "{body}");
        // Sorted, so the generated file does not churn between runs.
        let alpha_at = body.find(r#"["alpha""#).expect("alpha entry");
        let delta_at = body.find(r#"["delta""#).expect("delta entry");
        assert!(alpha_at < delta_at, "{body}");
    }

    #[test]
    fn scaffold_skips_containment_cycles() {
        // A parent tangled with its own submodule is never reported, so writing
        // an entry for it would be noise.
        let modules = module_set(&["nest", "nest::inner"]);
        let cycles = [cycle_of(&["nest", "nest::inner"])];
        let body = scaffold("my_crate", &modules, &cycles);
        assert!(!body.contains("[[check.allow-cycle]]"), "{body}");
    }

    #[test]
    fn scaffolded_cycle_baseline_round_trips() {
        // The generated text must load back as a valid rule set.
        let dir = tempfile::tempdir().expect("tempdir");
        let modules = module_set(&["alpha", "beta"]);
        let body = scaffold("my_crate", &modules, &[cycle_of(&["alpha", "beta"])]);
        let cfg = write_config(dir.path(), "crawk.toml", &body);
        let rules = RuleSet::load(&cfg, &modules).expect("generated config must load");
        assert!(rules.deny_cycles);
        assert_eq!(rules.allow_cycles.len(), 1);
        assert!(rules.allow_cycles[0].covers(&module_set(&["alpha", "beta"])));
    }

    #[test]
    fn scaffold_config_writes_when_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let modules = module_set(&["cli", "analyzer"]);
        let outcome =
            scaffold_config(dir.path(), None, "my_crate", &modules, &[]).expect("scaffold");
        assert_eq!(outcome.path, dir.path().join("crawk.toml"));
        assert_eq!(outcome.frozen_cycles, 0);
        let body = std::fs::read_to_string(&outcome.path).expect("read back");
        assert!(body.contains("[[check.layers]]"));
    }

    #[test]
    fn scaffold_config_reports_frozen_cycles() {
        let dir = tempfile::tempdir().expect("tempdir");
        let modules = module_set(&["alpha", "beta", "nest", "nest::inner"]);
        // Only the real tangle counts; the containment loop is not written.
        let cycles = [
            cycle_of(&["alpha", "beta"]),
            cycle_of(&["nest", "nest::inner"]),
        ];
        let outcome =
            scaffold_config(dir.path(), None, "my_crate", &modules, &cycles).expect("scaffold");
        assert_eq!(outcome.frozen_cycles, 1);
    }

    #[test]
    fn scaffold_config_honours_explicit_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("custom.toml");
        let modules = module_set(&["cli"]);
        let outcome = scaffold_config(dir.path(), Some(&target), "my_crate", &modules, &[])
            .expect("scaffold to explicit path");
        assert_eq!(outcome.path, target);
        assert!(target.is_file());
    }

    #[test]
    fn scaffold_config_refuses_existing_explicit_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = write_config(dir.path(), "custom.toml", "[check]\n");
        let modules = module_set(&["cli"]);
        // Explicit target that exists is refused even though discovery finds nothing.
        assert!(scaffold_config(dir.path(), Some(&target), "my_crate", &modules, &[]).is_err());
    }

    #[test]
    fn scaffold_config_refuses_when_config_exists() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_config(dir.path(), "crawk.toml", "[check]\n");
        let modules = module_set(&["cli"]);
        assert!(scaffold_config(dir.path(), None, "my_crate", &modules, &[]).is_err());
    }

    #[test]
    fn scaffold_config_refuses_when_hidden_config_exists() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_config(dir.path(), ".crawk.toml", "[check]\n");
        let modules = module_set(&["cli"]);
        // Reuses discovery, so the hidden name blocks scaffolding too.
        assert!(scaffold_config(dir.path(), None, "my_crate", &modules, &[]).is_err());
    }

    #[test]
    fn deny_rules_parse_with_explicit_subtree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.deny]]\nfrom = \"cli\"\nto = \"web::*\"\n",
        );
        let modules = module_set(&["cli", "web", "web::repo"]);
        let rules = RuleSet::load(&cfg, &modules).expect("load");
        assert_eq!(rules.deny.len(), 1);
        assert!(!rules.deny[0].from.subtree, "bare name stays exact");
        assert!(rules.deny[0].to.subtree, "explicit ::* marks the subtree");
        assert_eq!(rules.deny[0].display(), "deny cli -> web::*");
    }

    #[test]
    fn unknown_module_in_deny_is_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.deny]]\nfrom = \"clii\"\nto = \"web\"\n",
        );
        let modules = module_set(&["cli", "web"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should reject unknown module");
        assert!(matches!(
            &err,
            AnalysisError::UnknownRuleModule { module, rule }
                if module == "clii" && rule == "deny clii -> web"
        ));
    }

    #[test]
    fn unknown_key_in_deny_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.deny]]\nfrom = \"cli\"\nto = \"web\"\nvia = \"x\"\n",
        );
        let modules = module_set(&["cli", "web"]);
        assert!(RuleSet::load(&cfg, &modules).is_err());
    }

    #[test]
    fn deny_and_layers_coexist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.layers]]\nname = \"app\"\norder = [\"cli\", \"analyzer\"]\n\
             [[check.deny]]\nfrom = \"cli\"\nto = \"analyzer\"\n",
        );
        let modules = module_set(&["cli", "analyzer"]);
        let rules = RuleSet::load(&cfg, &modules).expect("load");
        assert_eq!(rules.layers.len(), 1);
        assert_eq!(rules.deny.len(), 1);
    }

    #[test]
    fn valid_config_loads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.layers]]\nname = \"app\"\norder = [\"cli\", \"analyzer\"]\n",
        );
        let modules = module_set(&["cli", "analyzer"]);
        assert!(RuleSet::load(&cfg, &modules).is_ok());
    }

    #[test]
    fn group_deny_same_layer_overrides_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Global default off; one group opts in, the other omits the key.
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-same-layer = false\n\
             [[check.layers]]\nname = \"strict\"\norder = [\"cli\"]\ndeny-same-layer = true\n\
             [[check.layers]]\nname = \"lax\"\norder = [\"analyzer\"]\n",
        );
        let modules = module_set(&["cli", "analyzer"]);
        let rules = RuleSet::load(&cfg, &modules).expect("load");
        assert!(rules.layers[0].deny_same_layer, "explicit override wins");
        assert!(
            !rules.layers[1].deny_same_layer,
            "omitted key stays default"
        );
    }

    #[test]
    fn global_deny_same_layer_propagates_to_groups() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Global default on; the group omits the key, so it inherits `true`.
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-same-layer = true\n\
             [[check.layers]]\nname = \"app\"\norder = [\"cli\"]\n",
        );
        let modules = module_set(&["cli"]);
        let rules = RuleSet::load(&cfg, &modules).expect("load");
        assert!(
            rules.layers[0].deny_same_layer,
            "group inherits the default"
        );
    }

    #[test]
    fn cycle_keys_default_to_off() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(dir.path(), "crawk.toml", "[check]\n");
        let rules = RuleSet::load(&cfg, &module_set(&["cli"])).expect("load");
        assert!(!rules.deny_cycles);
        assert!(!rules.deny_parent_child_cycles);
        assert!(rules.allow_cycles.is_empty());
    }

    #[test]
    fn cycle_keys_parse() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-cycles = true\ndeny-parent-child-cycles = true\n",
        );
        let rules = RuleSet::load(&cfg, &module_set(&["cli"])).expect("load");
        assert!(rules.deny_cycles);
        assert!(rules.deny_parent_child_cycles);
    }

    #[test]
    fn allow_cycle_parses_with_reason() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-cycles = true\n\
             [[check.allow-cycle]]\nmodules = [\"alpha\", \"beta\"]\nreason = \"tracked in #1\"\n",
        );
        let modules = module_set(&["alpha", "beta"]);
        let rules = RuleSet::load(&cfg, &modules).expect("load");
        assert_eq!(rules.allow_cycles.len(), 1);
        assert_eq!(
            rules.allow_cycles[0].reason.as_deref(),
            Some("tracked in #1")
        );
        assert_eq!(rules.allow_cycles[0].display(), "allow-cycle [alpha, beta]");
    }

    #[test]
    fn unknown_module_in_allow_cycle_is_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-cycles = true\n\
             [[check.allow-cycle]]\nmodules = [\"alpha\", \"alfa\"]\n",
        );
        let modules = module_set(&["alpha", "beta"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should reject unknown module");
        assert!(matches!(
            &err,
            AnalysisError::UnknownRuleModule { module, rule }
                if module == "alfa" && rule == "allow-cycle [alfa, alpha]"
        ));
    }

    #[test]
    fn single_module_allow_cycle_is_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A cycle needs two modules, so a one-element entry can never match.
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-cycles = true\n\
             [[check.allow-cycle]]\nmodules = [\"alpha\"]\n",
        );
        let modules = module_set(&["alpha", "beta"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should reject one-module entry");
        assert!(matches!(err, AnalysisError::RuleConfigError { .. }));
    }

    #[test]
    fn duplicate_allow_cycle_is_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-cycles = true\n\
             [[check.allow-cycle]]\nmodules = [\"alpha\", \"beta\"]\n\
             [[check.allow-cycle]]\nmodules = [\"beta\", \"alpha\"]\n",
        );
        let modules = module_set(&["alpha", "beta"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should reject duplicate entry");
        assert!(matches!(err, AnalysisError::RuleConfigError { .. }));
    }

    #[test]
    fn unknown_key_in_allow_cycle_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\n[[check.allow-cycle]]\nmodules = [\"alpha\", \"beta\"]\nwhy = \"x\"\n",
        );
        assert!(RuleSet::load(&cfg, &module_set(&["alpha", "beta"])).is_err());
    }

    #[test]
    fn group_can_override_global_deny_to_false() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Global default on; the group turns it back off for itself.
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[check]\ndeny-same-layer = true\n\
             [[check.layers]]\nname = \"app\"\norder = [\"cli\"]\ndeny-same-layer = false\n",
        );
        let modules = module_set(&["cli"]);
        let rules = RuleSet::load(&cfg, &modules).expect("load");
        assert!(
            !rules.layers[0].deny_same_layer,
            "group override to false wins"
        );
    }
}
