// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Helpers for working with policies as JSON-shaped value trees.
//!
//! `serde_json::Value` is used rather than `toml::Value` because a gate that
//! omits `with` deserializes to `serde_json::Value::Null`, and TOML has no
//! null value. The JSON tree can represent every policy state and is also the
//! canonical form used by the policy hash.

use repowill_model::Policy;
use serde_json::Value;

/// Arrays of tables that the policy schema keys by `id`.
///
/// The list is fixed by `plan/03-POLICY-AND-DATA.md` section 2 so that an
/// empty array still has unambiguous merge semantics.
pub const ID_ARRAY_PATHS: &[&str] = &[
    "gates",
    "protection.protected_paths",
    "docs.coupling",
    "docs.generated",
    "dependencies.exceptions",
    "waivers",
    "identity.allowed",
];

/// Serialize a policy into a value tree.
pub fn policy_to_value(policy: &Policy) -> Result<Value, String> {
    serde_json::to_value(policy).map_err(|e| e.to_string())
}

/// Deserialize a value tree into a policy.
pub fn value_to_policy(value: Value) -> Result<Policy, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Serialize a policy to TOML, omitting JSON null values (such as an absent
/// gate `with` table) so the result is valid TOML.
pub fn policy_to_toml(policy: &Policy) -> Result<String, String> {
    let value = policy_to_value(policy)?;
    let cleaned = strip_nulls(value);
    let toml_value = toml::Value::try_from(cleaned).map_err(|error| error.to_string())?;
    toml::to_string(&toml_value).map_err(|error| error.to_string())
}

fn strip_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut cleaned = serde_json::Map::new();
            for (key, item) in map {
                if item.is_null() {
                    continue;
                }
                cleaned.insert(key, strip_nulls(item));
            }
            Value::Object(cleaned)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(strip_nulls).collect()),
        other => other,
    }
}

/// Resolve a dotted key path through tables and id-bearing arrays.
///
/// `gates.tests.mode` resolves the `mode` field of the gate whose `id` is
/// `tests`. A numeric segment addresses a plain array index.
pub fn resolve_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        current = match current {
            Value::Object(table) => table.get(part)?,
            Value::Array(items) => match part.parse::<usize>() {
                Ok(index) => items.get(index)?,
                Err(_) => items
                    .iter()
                    .find(|item| item.get("id").and_then(Value::as_str) == Some(part))?,
            },
            _ => return None,
        };
    }
    Some(current)
}

/// True when `path` names one of the policy's id-keyed arrays.
pub fn is_id_array_path(path: &[String]) -> bool {
    let joined = path.join(".");
    ID_ARRAY_PATHS.contains(&joined.as_str())
}

/// Drop removal markers from an id array that is being introduced by a
/// layer: a `remove = true` entry removes an *inherited* entry and therefore
/// can never appear in an effective policy.
pub fn normalize_id_array(value: &Value) -> Value {
    let Some(items) = value.as_array() else {
        return value.clone();
    };
    Value::Array(
        items
            .iter()
            .filter(|item| !is_removal(item))
            .cloned()
            .collect(),
    )
}

/// True when an id-array entry only marks an inherited entry for removal.
pub fn is_removal(item: &Value) -> bool {
    item.get("remove").and_then(Value::as_bool) == Some(true)
}
