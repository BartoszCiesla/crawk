//! Config-file resolution, deserialization, and validation.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::warn;

use crate::error::{AnalysisError, Result};

use super::{LayerRule, ModulePattern, RuleSet};

/// Serde shape of the whole config file: a single `[check]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    check: RawCheck,
}

/// Serde shape of the `[check]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct RawCheck {
    layers: Vec<RawLayer>,
    strict_layers: bool,
    deny_same_layer: bool,
}

/// Serde shape of one `[[check.layers]]` group.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayer {
    name: String,
    order: Vec<String>,
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

    let plain = crate_root.join("crawk.toml");
    let hidden = crate_root.join(".crawk.toml");
    match (plain.is_file(), hidden.is_file()) {
        (true, true) => {
            warn!("both crawk.toml and .crawk.toml found, using crawk.toml");
            Ok(plain)
        }
        (true, false) => Ok(plain),
        (false, true) => Ok(hidden),
        (false, false) => Err(AnalysisError::RuleConfigError {
            path: crate_root.to_path_buf(),
            reason: "no crawk.toml or .crawk.toml found".to_owned(),
        }),
    }
}

impl RuleSet {
    /// Read, parse, and validate a rule-config file against the crate's modules.
    ///
    /// # Errors
    ///
    /// - [`AnalysisError::RuleConfigError`] — file unreadable, malformed TOML,
    ///   overlapping layer groups, or (under `strict-layers`) an uncovered module.
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
        let rules = Self::from_raw(raw.check);
        rules.validate(path, modules)?;
        Ok(rules)
    }

    /// Convert the deserialized shape into the validated in-memory form.
    fn from_raw(raw: RawCheck) -> Self {
        let layers = raw
            .layers
            .into_iter()
            .map(|raw_layer| LayerRule {
                name: raw_layer.name,
                order: raw_layer
                    .order
                    .iter()
                    .map(|entry| ModulePattern::parse_subtree(entry))
                    .collect(),
            })
            .collect();
        Self {
            layers,
            strict_layers: raw.strict_layers,
            deny_same_layer: raw.deny_same_layer,
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
        // 2. Groups must be disjoint; under strict mode, every module covered.
        for module in modules {
            match self.resolve_layer(module) {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if self.strict_layers {
                        return Err(AnalysisError::RuleConfigError {
                            path: path.to_path_buf(),
                            reason: format!(
                                "strict-layers: module '{module}' is not assigned to any layer"
                            ),
                        });
                    }
                }
                Err(amb) => {
                    return Err(AnalysisError::RuleConfigError {
                        path: path.to_path_buf(),
                        reason: format!(
                            "module '{}' matches two layer groups ('{}' and '{}')",
                            amb.module, amb.group_a, amb.group_b
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
    fn overlapping_groups_are_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = write_config(
            dir.path(),
            "crawk.toml",
            "[[check.layers]]\nname = \"a\"\norder = [\"cli\", \"parser\"]\n\
             [[check.layers]]\nname = \"b\"\norder = [\"parser\", \"discover\"]\n",
        );
        let modules = module_set(&["cli", "parser", "discover"]);
        let err = RuleSet::load(&cfg, &modules).expect_err("should reject overlap");
        assert!(matches!(err, AnalysisError::RuleConfigError { .. }));
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
}
