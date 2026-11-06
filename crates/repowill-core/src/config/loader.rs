// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Layered policy loading and value-level merging.
//!
//! The effective policy is the merge of the built-in defaults, the `extends`
//! profile files in order, `.repowill/policy.toml`, and finally an optional
//! `.repowill/local.toml`. Layers are merged as JSON-shaped value trees
//! before being deserialized, which preserves the difference between an
//! omitted key and an explicitly set default value, and lets id-bearing
//! arrays merge field by field.
//!
//! See `plan/03-POLICY-AND-DATA.md` sections 2 and 10.

use crate::config::migrate::SUPPORTED_SCHEMA_VERSION;
use crate::config::strictness::weakening_changes;
use crate::config::validate::{Diagnostic, Severity};
use crate::config::value::{
    ID_ARRAY_PATHS, is_id_array_path, normalize_id_array, policy_to_value, resolve_path,
    value_to_policy,
};
use repowill_model::Policy;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Diagnostic code for a local layer that weakens the committed policy.
pub const E_WEAKENING: &str = "RW-E2104";
/// Diagnostic code for a change to a key frozen by `[lock].keys`.
pub const E_LOCKED: &str = "RW-E2105";
/// Diagnostic code for an `extends` file that cannot be resolved.
pub const E_EXTENDS_MISSING: &str = "RW-E2101";
/// Diagnostic code for an `extends` path that leaves the policy directory.
pub const E_EXTENDS_ESCAPE: &str = "RW-E2102";
/// Diagnostic code for an `extends` cycle.
pub const E_EXTENDS_CYCLE: &str = "RW-E2103";
/// Diagnostic code for an unsupported policy schema version.
pub const E_SCHEMA_VERSION: &str = "RW-E2001";

/// The built-in defaults layer (equal to the Recommended preset without
/// stack gates, per T-01.1.1).
pub fn builtin_policy() -> Policy {
    Policy::default()
}

/// The built-in defaults as a value tree.
pub fn builtin_value() -> Result<Value, String> {
    policy_to_value(&builtin_policy())
}

/// Parse TOML source into a JSON-shaped value tree.
///
/// A leading UTF-8 BOM is tolerated. TOML datetime values are rejected
/// because the policy schema stores every date as a string and JSON has no
/// datetime representation.
pub fn parse_toml_value(src: &str) -> Result<Value, String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let value: toml::Value = toml::from_str(src).map_err(|e| e.to_string())?;
    toml_to_json(value)
}

fn toml_to_json(value: toml::Value) -> Result<Value, String> {
    Ok(match value {
        toml::Value::String(text) => Value::String(text),
        toml::Value::Integer(number) => Value::Number(number.into()),
        toml::Value::Float(number) => {
            let Some(number) = serde_json::Number::from_f64(number) else {
                return Err("policy values must be finite JSON numbers".to_string());
            };
            Value::Number(number)
        }
        toml::Value::Boolean(flag) => Value::Bool(flag),
        toml::Value::Datetime(_) => {
            return Err("policy values must not contain TOML datetimes; use strings".to_string());
        }
        toml::Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(toml_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        toml::Value::Table(table) => {
            let mut object = Map::new();
            for (key, item) in table {
                object.insert(key, toml_to_json(item)?);
            }
            Value::Object(object)
        }
    })
}

/// Parse a complete policy document without applying built-in defaults.
///
/// This is the entry point for callers that need to inspect exactly what a
/// file says (including a missing or unsupported version), for example the
/// diagnostics renderer.
pub fn parse_policy_str(src: &str) -> Result<Policy, String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    toml::from_str(src).map_err(|e| e.to_string())
}

fn check_layer_version(layer: &Value) -> Result<(), String> {
    let version = layer.get("schema_version").ok_or_else(|| {
        format!("{E_SCHEMA_VERSION}: every policy layer must declare schema_version = 1")
    })?;
    let Some(version) = version.as_u64() else {
        return Err(format!(
            "{E_SCHEMA_VERSION}: schema_version must be a positive integer"
        ));
    };
    if version > u64::from(SUPPORTED_SCHEMA_VERSION) {
        return Err(format!(
            "{E_SCHEMA_VERSION}: policy schema version {version} is newer than the supported \
             version {SUPPORTED_SCHEMA_VERSION}; update RepoWill"
        ));
    }
    if version < 1 {
        return Err(format!(
            "{E_SCHEMA_VERSION}: policy schema version {version} is not supported; run \
             `repowill policy migrate` with a RepoWill release that can read it"
        ));
    }
    Ok(())
}

fn extends_of(layer: &Value) -> Result<Vec<String>, String> {
    let Some(value) = layer.get("extends") else {
        return Ok(vec![]);
    };
    let Some(items) = value.as_array() else {
        return Err("extends must be an array of profile paths".to_string());
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err("extends entries must be strings".to_string());
        };
        if text.is_empty() {
            return Err("extends entries must not be empty".to_string());
        }
        out.push(text.to_string());
    }
    Ok(out)
}

/// Merge `layer` into `base` according to section 2 merge rules.
///
/// The merge is transactional: `base` is left untouched when a locked key
/// changed or the layer is not a table. Locked keys are those present in
/// `base` before this layer; a lock added by `layer` only applies to *later*
/// layers.
pub fn merge_values(base: &mut Value, layer: &Value) -> Result<(), String> {
    if !base.is_object() {
        return Err("internal error: base policy is not a table".to_string());
    }
    if !layer.is_object() {
        return Err("policy layer must be a table".to_string());
    }

    let locked = lock_keys(base);
    let mut trial = base.clone();
    merge_object(
        trial
            .as_object_mut()
            .expect("checked base policy is a table"),
        layer.as_object().expect("checked policy layer is a table"),
        &mut Vec::new(),
    );

    for key in &locked {
        if resolve_path(&trial, key) != resolve_path(base, key) {
            return Err(format!("{E_LOCKED}: locked policy key changed: {key}"));
        }
    }
    *base = trial;
    Ok(())
}

fn lock_keys(value: &Value) -> Vec<String> {
    value
        .get("lock")
        .and_then(|lock| lock.get("keys"))
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn merge_object(base: &mut Map<String, Value>, layer: &Map<String, Value>, path: &mut Vec<String>) {
    for (key, value) in layer {
        path.push(key.clone());
        if path.join(".") == "lock.keys" {
            merge_lock_keys(base, key, value);
            path.pop();
            continue;
        }
        match base.get_mut(key) {
            Some(existing) => merge_value(existing, value, path),
            None => {
                let value = if is_id_array_path(path) {
                    normalize_id_array(value)
                } else {
                    value.clone()
                };
                base.insert(key.clone(), value);
            }
        }
        path.pop();
    }
}

fn merge_lock_keys(base: &mut Map<String, Value>, key: &str, value: &Value) {
    let Value::Array(layer_keys) = value else {
        base.insert(key.to_string(), value.clone());
        return;
    };
    match base.get_mut(key) {
        Some(Value::Array(base_keys)) => {
            for item in layer_keys {
                if !base_keys.contains(item) {
                    base_keys.push(item.clone());
                }
            }
        }
        _ => {
            base.insert(key.to_string(), value.clone());
        }
    }
}

fn merge_value(base: &mut Value, layer: &Value, path: &mut Vec<String>) {
    match (base, layer) {
        (Value::Object(base), Value::Object(layer)) => merge_object(base, layer, path),
        (Value::Array(base), Value::Array(layer)) if is_id_array_path(path) => {
            merge_id_array(base, layer);
        }
        (base, layer) => *base = layer.clone(),
    }
}

fn merge_id_array(base: &mut Vec<Value>, layer: &[Value]) {
    for entry in layer {
        let Some(object) = entry.as_object() else {
            base.push(entry.clone());
            continue;
        };
        let Some(id) = object.get("id").and_then(Value::as_str) else {
            base.push(entry.clone());
            continue;
        };
        if object.get("remove").and_then(Value::as_bool) == Some(true) {
            base.retain(|item| item.get("id").and_then(Value::as_str) != Some(id));
            continue;
        }
        match base
            .iter_mut()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        {
            Some(existing) => {
                // Fields the layer does not set stay inherited. Nested arrays
                // inside an entry are plain arrays and replace wholesale.
                merge_value(existing, entry, &mut Vec::new());
            }
            None => base.push(entry.clone()),
        }
    }
}

/// Load a policy string as `built-in defaults + the given layer`.
///
/// `extends` cannot be resolved without a file path; use
/// [`load_policy_file`] when the document declares profiles.
pub fn load_policy_str(src: &str) -> Result<Policy, String> {
    let layer = parse_toml_value(src)?;
    check_layer_version(&layer)?;
    if !extends_of(&layer)?.is_empty() {
        return Err(
            "extends can only be resolved from a file; use load_policy_file instead".to_string(),
        );
    }
    let mut value = builtin_value()?;
    merge_values(&mut value, &layer)?;
    value_to_policy(value)
}

/// Load an ordered list of partial layers over the built-in defaults.
pub fn load_layers(sources: &[&str]) -> Result<Policy, String> {
    let mut value = builtin_value()?;
    for src in sources {
        let layer = parse_toml_value(src)?;
        check_layer_version(&layer)?;
        merge_values(&mut value, &layer)?;
    }
    value_to_policy(value)
}

/// Load a policy file, resolving `extends` profiles first.
///
/// `extends` paths are relative to the file that declares them and must stay
/// below the directory of the top-level policy file. Cycles and missing
/// profiles are errors (fail closed).
pub fn load_policy_file(path: &Path) -> Result<Policy, String> {
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let root = std::fs::canonicalize(root).map_err(|e| {
        format!(
            "{E_EXTENDS_MISSING}: cannot resolve policy directory {}: {e}",
            root.display()
        )
    })?;
    let mut stack = Vec::new();
    let (layers, extends) = collect_layers(path, &root, &mut stack)?;
    let mut value = builtin_value()?;
    for layer in layers {
        merge_values(&mut value, &layer)?;
    }
    if !extends.is_empty() {
        value["extends"] = Value::Array(extends.into_iter().map(Value::String).collect());
    }
    value_to_policy(value)
}

fn collect_layers(
    path: &Path,
    root: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<(Vec<Value>, Vec<String>), String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| {
        format!(
            "{E_EXTENDS_MISSING}: cannot read policy file {}: {e}",
            path.display()
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(format!(
            "{E_EXTENDS_ESCAPE}: policy file {} is outside {}; extends must stay inside the \
             policy directory",
            path.display(),
            root.display()
        ));
    }
    if stack.contains(&canonical) {
        return Err(format!(
            "{E_EXTENDS_CYCLE}: extends cycle detected at {}",
            path.display()
        ));
    }
    stack.push(canonical.clone());

    let src = std::fs::read_to_string(&canonical).map_err(|e| {
        format!(
            "{E_EXTENDS_MISSING}: cannot read policy file {}: {e}",
            path.display()
        )
    })?;
    let layer = parse_toml_value(&src)?;
    check_layer_version(&layer)?;
    let extends = extends_of(&layer)?;

    let parent = canonical.parent().unwrap_or(root).to_path_buf();
    let mut layers = Vec::new();
    for profile in &extends {
        let profile_path = parent.join(profile);
        let (mut nested, _) = collect_layers(&profile_path, root, stack)?;
        layers.append(&mut nested);
    }

    stack.pop();
    let mut own = layer;
    if let Some(object) = own.as_object_mut() {
        object.remove("extends");
    }
    layers.push(own);
    Ok((layers, extends))
}

/// Merge a fully resolved policy layer, enforcing `[lock]` freezes.
///
/// Both layers are converted to value trees and merged with the normal
/// id-aware rules, so only the fields present in `layer` replace inherited
/// values.
pub fn merge_layer(base: &mut Policy, layer: &Policy) -> Result<(), String> {
    let mut value = policy_to_value(base)?;
    let layer = policy_to_value(layer)?;
    merge_values(&mut value, &layer)?;
    *base = value_to_policy(value)?;
    Ok(())
}

/// Merge and validate a fully resolved policy layer.
///
/// The merged trial is validated before it replaces `base` (R7-2). Errors are
/// returned as a semicolon-separated list prefixed with their diagnostic
/// codes.
pub fn merge_layer_checked(base: &mut Policy, layer: &Policy) -> Result<(), String> {
    let mut trial = base.clone();
    merge_layer(&mut trial, layer)?;
    let errors = validation_errors(&crate::config::validate::validate(&trial));
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    *base = trial;
    Ok(())
}

/// Apply a local layer, which may only tighten the committed policy.
///
/// The effective local policy must not contain any weakening change relative
/// to `base` unless the committed policy already sets
/// `protection.policy.allow_local_weakening = true`. A local layer cannot
/// grant itself that permission, because the permission itself is a
/// weakening change (GD-07).
pub fn load_local(base: &Policy, local_src: &str) -> Result<Policy, String> {
    let layer = parse_toml_value(local_src)?;
    check_layer_version(&layer)?;
    if !extends_of(&layer)?.is_empty() {
        return Err("local.toml must not declare extends".to_string());
    }
    let mut value = policy_to_value(base)?;
    merge_values(&mut value, &layer)?;
    let local = value_to_policy(value)?;
    let errors = validation_errors(&crate::config::validate::validate(&local));
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    let weakening = weakening_changes(base, &local);
    if !weakening.is_empty() && !base.protection.policy.allow_local_weakening {
        let paths: Vec<&str> = weakening
            .iter()
            .map(|change| change.path.as_str())
            .collect();
        return Err(format!(
            "{E_WEAKENING}: local.toml weakens the committed policy ({} change(s): {}); \
             request a policy change instead",
            weakening.len(),
            paths.join(", ")
        ));
    }
    Ok(local)
}

fn validation_errors(diagnostics: &[Diagnostic]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .filter(|message| seen.insert(message.clone()))
        .collect()
}

/// Names of every id-keyed array in the schema, for callers that need to
/// enumerate merge rules (documentation and tests).
pub fn id_array_paths() -> &'static [&'static str] {
    ID_ARRAY_PATHS
}

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::{Gate, GatePaths, Mode};
    use std::fs;
    use tempfile::tempdir;

    fn gate(id: &str) -> Gate {
        Gate {
            id: id.into(),
            name: format!("Gate {id}"),
            description: String::new(),
            uses: "builtin:dco".into(),
            with: serde_json::Value::Null,
            stages: vec!["pre-commit".into()],
            mode: Mode::Block,
            stage_modes: Default::default(),
            trigger: "auto".into(),
            on_error: "block".into(),
            timeout: "10m".into(),
            paths: GatePaths {
                include: vec!["**".into()],
                exclude: vec![],
            },
            run_always: false,
            depends_on: vec![],
            profiles: vec![],
            branches: vec![],
            only_in: "any".into(),
            os: vec![],
            tags: vec![],
            locked: false,
            remove: false,
        }
    }

    #[test]
    fn builtins_are_applied() {
        let policy = load_policy_str("schema_version = 1\n").unwrap();
        assert_eq!(policy, Policy::default());
    }

    #[test]
    fn field_level_gate_merge() {
        let base_src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"Original\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\nmode = \"block\"\n";
        let layer_src = "schema_version = 1\n[[gates]]\nid = \"t\"\nmode = \"warn\"\n";
        let mut value = builtin_value().unwrap();
        merge_values(&mut value, &parse_toml_value(base_src).unwrap()).unwrap();
        merge_values(&mut value, &parse_toml_value(layer_src).unwrap()).unwrap();
        let policy = value_to_policy(value).unwrap();
        assert_eq!(policy.gates[0].name, "Original");
        assert_eq!(policy.gates[0].mode, Mode::Warn);
    }

    #[test]
    fn exceptions_remove() {
        let base_src = "schema_version = 1\n[[dependencies.exceptions]]\nid = \"ex\"\npackage = \"npm:x\"\nversions = \"*\"\nallow = [\"license\"]\nreason = \"r\"\nexpires = \"2026-12-31\"\nrequest = \"\"\n";
        let layer_src =
            "schema_version = 1\n[[dependencies.exceptions]]\nid = \"ex\"\nremove = true\n";
        let mut value = builtin_value().unwrap();
        merge_values(&mut value, &parse_toml_value(base_src).unwrap()).unwrap();
        merge_values(&mut value, &parse_toml_value(layer_src).unwrap()).unwrap();
        let policy = value_to_policy(value).unwrap();
        assert!(policy.dependencies.exceptions.is_empty());
    }

    #[test]
    fn lock_enforced_checked() {
        let mut base = Policy::default();
        base.lock.keys = vec!["guards.main_protection".into()];
        let mut layer = Policy::default();
        layer.guards.main_protection = Mode::Off;
        assert!(merge_layer_checked(&mut base, &layer).is_err());
    }

    #[test]
    fn lock_added_in_same_layer_applies_later() {
        let layer_one = "schema_version = 1\n[lock]\nkeys = [\"guards.main_protection\"]\n";
        let layer_two = "schema_version = 1\n[guards]\nmain_protection = \"off\"\n";
        let mut value = builtin_value().unwrap();
        merge_values(&mut value, &parse_toml_value(layer_one).unwrap()).unwrap();
        let err = merge_values(&mut value, &parse_toml_value(layer_two).unwrap());
        assert!(err.is_err());
    }

    #[test]
    fn merge_layer_checked_validates_trial() {
        let mut base = Policy::default();
        let mut layer = Policy::default();
        layer.gates.push(gate("dup"));
        base.gates.push(gate("dup"));
        layer.gates[0].name = "changed".into();
        // Duplicate gate id in merged effective policy is caught by validation.
        let mut invalid_base = Policy::default();
        invalid_base.gates.push(gate("dup"));
        let mut invalid_layer = Policy::default();
        invalid_layer.gates.push(gate("dup"));
        invalid_layer.gates[0].stages = vec![];
        assert!(merge_layer_checked(&mut invalid_base, &invalid_layer).is_err());
    }

    #[test]
    fn local_tighten_only_enforced_load() {
        let base_src = "schema_version = 1\n";
        let base = load_policy_str(base_src).unwrap();
        let local_src = "schema_version = 1\n[guards]\nmain_protection = \"off\"\n";
        assert!(load_local(&base, local_src).is_err());
    }

    #[test]
    fn local_weakening_allowed_when_committed() {
        let base_src = "schema_version = 1\n[protection.policy]\nallow_local_weakening = true\n";
        let base = load_policy_str(base_src).unwrap();
        let local_src = "schema_version = 1\n[guards]\nmain_protection = \"off\"\n";
        assert!(load_local(&base, local_src).is_ok());
    }

    #[test]
    fn extends_resolution_and_cycle() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let profile = root.join("profile.toml");
        fs::write(
            &profile,
            "schema_version = 1\n[project]\nname = \"profiled\"\n",
        )
        .unwrap();
        let policy_path = root.join("policy.toml");
        fs::write(
            &policy_path,
            "schema_version = 1\nextends = [\"profile.toml\"]\n",
        )
        .unwrap();
        let policy = load_policy_file(&policy_path).unwrap();
        assert_eq!(policy.project.name, "profiled");

        // Cycle.
        fs::write(
            &profile,
            "schema_version = 1\nextends = [\"policy.toml\"]\n",
        )
        .unwrap();
        fs::write(
            &policy_path,
            "schema_version = 1\nextends = [\"profile.toml\"]\n",
        )
        .unwrap();
        assert!(load_policy_file(&policy_path).is_err());
    }

    #[test]
    fn extends_escape_rejected() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let outside = dir.path().parent().unwrap().join("outside.toml");
        fs::write(&outside, "schema_version = 1\n").unwrap();
        let policy_path = root.join("policy.toml");
        fs::write(
            &policy_path,
            "schema_version = 1\nextends = [\"../outside.toml\"]\n",
        )
        .unwrap();
        assert!(load_policy_file(&policy_path).is_err());
    }

    #[test]
    fn id_array_paths_listed() {
        assert!(id_array_paths().contains(&"dependencies.exceptions"));
    }
}
