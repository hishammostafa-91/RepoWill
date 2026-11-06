// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Typed editing helpers for policy TOML documents.
//!
//! Every helper parses the document with `toml_edit` (so comments and
//! ordering are preserved), applies the requested change, and then validates
//! the resulting effective policy against the built-in defaults. A change that
//! produces an invalid policy is rejected before the source is returned.

use crate::config::loader::{builtin_value, merge_values, parse_toml_value};
use crate::config::validate::{self, Severity};
use crate::config::value::value_to_policy;
use repowill_model::{Gate, Mode};
use serde_json::Value as JsonValue;
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value as TomlValue};

/// A typed value accepted by [`set_key`].
#[derive(Debug, Clone, PartialEq)]
pub enum EditValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Strings(Vec<String>),
    /// A raw TOML value such as `true`, `["a", "b"]`, or `{include = ["**"]}`.
    Toml(String),
}

impl From<i64> for EditValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for EditValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<Vec<String>> for EditValue {
    fn from(value: Vec<String>) -> Self {
        Self::Strings(value)
    }
}

impl From<&str> for EditValue {
    fn from(value: &str) -> Self {
        if value.parse::<TomlValue>().is_ok() {
            Self::Toml(value.to_string())
        } else {
            Self::String(value.to_string())
        }
    }
}

impl From<String> for EditValue {
    fn from(value: String) -> Self {
        if value.parse::<TomlValue>().is_ok() {
            Self::Toml(value)
        } else {
            Self::String(value)
        }
    }
}

impl From<&str> for GatePosition {
    fn from(value: &str) -> Self {
        Self::Before(value.to_string())
    }
}

impl From<String> for GatePosition {
    fn from(value: String) -> Self {
        Self::Before(value)
    }
}

/// Where a gate should be inserted or moved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatePosition {
    Start,
    End,
    Before(String),
    After(String),
    Index(usize),
}

/// Set a dotted policy key to a typed value.
///
/// Dotted paths walk through tables and id-keyed arrays of tables (`gates`,
/// `protection.protected_paths`, ...). A segment that names an id array selects
/// the entry whose `id` field matches the next segment. Plain arrays can only be
/// replaced as a whole through [`EditValue::Toml`].
pub fn set_key(src: &str, key: &str, value: impl Into<EditValue>) -> Result<String, String> {
    let value = value.into();
    let mut doc = parse_doc(src)?;
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|part| part.is_empty()) {
        return Err("policy key must not contain empty segments".to_string());
    }
    set_nested(doc.as_table_mut(), &parts, &value)?;
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Remove a dotted policy key.
pub fn unset_key(src: &str, key: &str) -> Result<String, String> {
    let mut doc = parse_doc(src)?;
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|part| part.is_empty()) {
        return Err("policy key must not contain empty segments".to_string());
    }
    if !unset_nested(doc.as_table_mut(), &parts) {
        return Err(format!("policy key not found: {key}"));
    }
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Add a gate at the requested position.
pub fn add_gate(
    src: &str,
    gate: &Gate,
    position: impl Into<GatePosition>,
) -> Result<String, String> {
    let position = position.into();
    let mut doc = parse_doc(src)?;
    let arr = ensure_gates_array(&mut doc)?;
    if arr
        .iter()
        .any(|table| table.get("id").and_then(|v| v.as_str()) == Some(&gate.id))
    {
        return Err(format!("gate id already exists: {}", gate.id));
    }
    let table = gate_table(gate)?;
    let index = resolve_insert_index(arr, &position)?;
    arr.insert(index, table);
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Update fields of a gate identified by id.
pub fn update_gate(src: &str, id: &str, updates: &[(&str, EditValue)]) -> Result<String, String> {
    let mut doc = parse_doc(src)?;
    let arr = doc
        .get_mut("gates")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "no gates array".to_string())?;
    let table = arr
        .iter_mut()
        .find(|table| table.get("id").and_then(|v| v.as_str()) == Some(id))
        .ok_or_else(|| format!("gate not found: {id}"))?;
    for (path, value) in updates {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.iter().any(|part| part.is_empty()) {
            return Err("gate key must not contain empty segments".to_string());
        }
        set_in_table(table, &parts, value)?;
    }
    if updates.iter().any(|(path, _)| *path == "id") {
        let new_id = updates
            .iter()
            .find(|(path, _)| *path == "id")
            .and_then(|(_, value)| match value {
                EditValue::String(text) => Some(text.clone()),
                EditValue::Toml(text) => Some(text.trim_matches('"').to_string()),
                _ => None,
            })
            .unwrap_or_else(|| id.to_string());
        if new_id != id
            && arr
                .iter()
                .any(|table| table.get("id").and_then(|v| v.as_str()) == Some(&new_id))
        {
            return Err(format!("gate id already exists: {new_id}"));
        }
    }
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Remove a gate identified by id.
pub fn remove_gate(src: &str, id: &str) -> Result<String, String> {
    let mut doc = parse_doc(src)?;
    let arr = doc
        .get_mut("gates")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "no gates array".to_string())?;
    let index = arr
        .iter()
        .position(|table| table.get("id").and_then(|v| v.as_str()) == Some(id))
        .ok_or_else(|| format!("gate not found: {id}"))?;
    arr.remove(index);
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Move a gate to a new position.
pub fn move_gate(src: &str, id: &str, position: impl Into<GatePosition>) -> Result<String, String> {
    let position = position.into();
    let mut doc = parse_doc(src)?;
    let arr = doc
        .get_mut("gates")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "no gates array".to_string())?;
    let source_index = arr
        .iter()
        .position(|table| table.get("id").and_then(|v| v.as_str()) == Some(id))
        .ok_or_else(|| format!("gate not found: {id}"))?;
    let table = arr.remove(source_index);
    let remaining = arr.len();
    let target_index = match &position {
        GatePosition::Start => 0,
        GatePosition::End => remaining,
        GatePosition::Index(index) => {
            if *index > remaining {
                return Err(format!("gate index out of range: {index}"));
            }
            *index
        }
        GatePosition::Before(target) => {
            if target == id {
                return Err("cannot move a gate relative to itself".to_string());
            }
            arr.iter()
                .position(|table| table.get("id").and_then(|v| v.as_str()) == Some(target))
                .ok_or_else(|| format!("gate not found: {target}"))?
        }
        GatePosition::After(target) => {
            if target == id {
                return Err("cannot move a gate relative to itself".to_string());
            }
            let pos = arr
                .iter()
                .position(|table| table.get("id").and_then(|v| v.as_str()) == Some(target))
                .ok_or_else(|| format!("gate not found: {target}"))?;
            pos + 1
        }
    };
    arr.insert(target_index, table);
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Duplicate a gate under a new id.
pub fn duplicate_gate(
    src: &str,
    id: &str,
    new_id: &str,
    position: impl Into<GatePosition>,
) -> Result<String, String> {
    let position = position.into();
    let mut doc = parse_doc(src)?;
    let arr = doc
        .get_mut("gates")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "no gates array".to_string())?;
    if arr
        .iter()
        .any(|table| table.get("id").and_then(|v| v.as_str()) == Some(new_id))
    {
        return Err(format!("gate id already exists: {new_id}"));
    }
    let source = arr
        .iter()
        .find(|table| table.get("id").and_then(|v| v.as_str()) == Some(id))
        .ok_or_else(|| format!("gate not found: {id}"))?
        .clone();
    let mut duplicated = source;
    duplicated["id"] = toml_edit::value(new_id);
    let index = resolve_insert_index(arr, &position)?;
    arr.insert(index, duplicated);
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Set a gate's mode.
pub fn set_gate_mode(src: &str, id: &str, mode: Mode) -> Result<String, String> {
    let mut doc = parse_doc(src)?;
    let arr = doc
        .get_mut("gates")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "no gates array".to_string())?;
    let table = arr
        .iter_mut()
        .find(|table| table.get("id").and_then(|v| v.as_str()) == Some(id))
        .ok_or_else(|| format!("gate not found: {id}"))?;
    table["mode"] = toml_edit::value(match mode {
        Mode::Block => "block",
        Mode::Warn => "warn",
        Mode::Off => "off",
    });
    let out = doc.to_string();
    validate_edited(&out)?;
    Ok(out)
}

/// Enable or disable a gate by setting its mode.
pub fn set_gate_enabled(src: &str, id: &str, enabled: bool) -> Result<String, String> {
    set_gate_mode(src, id, if enabled { Mode::Block } else { Mode::Off })
}

pub fn enable_gate(src: &str, id: &str) -> Result<String, String> {
    set_gate_enabled(src, id, true)
}

pub fn disable_gate(src: &str, id: &str) -> Result<String, String> {
    set_gate_enabled(src, id, false)
}

fn parse_doc(src: &str) -> Result<DocumentMut, String> {
    src.parse::<DocumentMut>()
        .map_err(|error| error.to_string())
}

fn ensure_gates_array(doc: &mut DocumentMut) -> Result<&mut toml_edit::ArrayOfTables, String> {
    if doc.get("gates").is_none() {
        doc["gates"] = Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }
    doc.get_mut("gates")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "gates must be an array of tables".to_string())
}

fn set_nested(table: &mut Table, parts: &[&str], value: &EditValue) -> Result<(), String> {
    if parts.is_empty() {
        return Ok(());
    }
    if parts.len() == 1 {
        let item = edit_value_to_item(value)?;
        table.insert(parts[0], item);
        return Ok(());
    }
    let key = parts[0];
    let item = table.get_mut(key);
    match item {
        Some(item) if item.is_table() => {
            let nested = item.as_table_mut().expect("checked table");
            set_nested(nested, &parts[1..], value)
        }
        Some(item) if item.is_array_of_tables() => {
            let arr = item.as_array_of_tables_mut().expect("checked array");
            let next = parts[1];
            let index = index_in_id_array(arr, next)?;
            let nested = arr
                .get_mut(index)
                .ok_or_else(|| format!("entry not found: {next}"))?;
            set_nested(nested, &parts[2..], value)
        }
        Some(_) => Err(format!("`{key}` is not a table")),
        None => {
            let mut nested = Table::new();
            nested.set_implicit(true);
            table.insert(key, Item::Table(nested));
            let item = table.get_mut(key).expect("just inserted");
            let nested = item.as_table_mut().expect("checked table");
            set_nested(nested, &parts[1..], value)
        }
    }
}

fn unset_nested(table: &mut Table, parts: &[&str]) -> bool {
    if parts.is_empty() {
        return false;
    }
    if parts.len() == 1 {
        return table.remove(parts[0]).is_some();
    }
    let Some(item) = table.get_mut(parts[0]) else {
        return false;
    };
    if let Some(nested) = item.as_table_mut() {
        return unset_nested(nested, &parts[1..]);
    }
    if let Some(arr) = item.as_array_of_tables_mut() {
        let next = parts[1];
        let Ok(index) = index_in_id_array(arr, next) else {
            return false;
        };
        if let Some(nested) = arr.get_mut(index) {
            if parts.len() == 2 {
                // Cannot remove a whole entry through unset_key; callers use remove_gate.
                return false;
            }
            return unset_nested(nested, &parts[2..]);
        }
    }
    false
}

fn set_in_table(table: &mut Table, parts: &[&str], value: &EditValue) -> Result<(), String> {
    if parts.is_empty() {
        return Ok(());
    }
    if parts.len() == 1 {
        table.insert(parts[0], edit_value_to_item(value)?);
        return Ok(());
    }
    let key = parts[0];
    let item = table.get_mut(key);
    match item {
        Some(item) if item.is_table() => {
            let nested = item.as_table_mut().expect("checked table");
            set_in_table(nested, &parts[1..], value)
        }
        Some(_) => Err(format!("`{key}` is not a table")),
        None => {
            let mut nested = Table::new();
            nested.set_implicit(true);
            table.insert(key, Item::Table(nested));
            let item = table.get_mut(key).expect("just inserted");
            let nested = item.as_table_mut().expect("checked table");
            set_in_table(nested, &parts[1..], value)
        }
    }
}

fn index_in_id_array(arr: &toml_edit::ArrayOfTables, key: &str) -> Result<usize, String> {
    if let Ok(index) = key.parse::<usize>() {
        if index < arr.len() {
            return Ok(index);
        }
        return Err(format!("array index out of range: {index}"));
    }
    arr.iter()
        .position(|table| table.get("id").and_then(|v| v.as_str()) == Some(key))
        .ok_or_else(|| format!("entry not found: {key}"))
}

fn edit_value_to_item(value: &EditValue) -> Result<Item, String> {
    Ok(match value {
        EditValue::String(text) => toml_edit::value(text.clone()),
        EditValue::Integer(number) => toml_edit::value(*number),
        EditValue::Boolean(flag) => toml_edit::value(*flag),
        EditValue::Strings(values) => {
            let mut array = toml_edit::Array::new();
            for item in values {
                array.push(item.as_str());
            }
            Item::Value(TomlValue::Array(array))
        }
        EditValue::Toml(text) => {
            let parsed: TomlValue = text
                .parse()
                .map_err(|error: toml_edit::TomlError| error.to_string())?;
            Item::Value(parsed)
        }
    })
}

fn gate_table(gate: &Gate) -> Result<Table, String> {
    let mut table = Table::new();
    table["id"] = toml_edit::value(gate.id.clone());
    table["name"] = toml_edit::value(gate.name.clone());
    if !gate.description.is_empty() {
        table["description"] = toml_edit::value(gate.description.clone());
    }
    table["uses"] = toml_edit::value(gate.uses.clone());
    if !gate.with.is_null() {
        table["with"] = Item::Value(json_to_toml(&gate.with)?);
    }
    let mut stages = toml_edit::Array::new();
    for stage in &gate.stages {
        stages.push(stage.as_str());
    }
    table["stages"] = Item::Value(TomlValue::Array(stages));
    table["mode"] = toml_edit::value(match gate.mode {
        Mode::Block => "block",
        Mode::Warn => "warn",
        Mode::Off => "off",
    });
    if !gate.stage_modes.is_empty() {
        let mut stage_modes = InlineTable::new();
        for (stage, mode) in &gate.stage_modes {
            stage_modes.insert(
                stage,
                TomlValue::String(toml_edit::Formatted::new(
                    match mode {
                        Mode::Block => "block",
                        Mode::Warn => "warn",
                        Mode::Off => "off",
                    }
                    .to_string(),
                )),
            );
        }
        table["stage_modes"] = Item::Value(TomlValue::InlineTable(stage_modes));
    }
    table["trigger"] = toml_edit::value(gate.trigger.clone());
    table["on_error"] = toml_edit::value(gate.on_error.clone());
    table["timeout"] = toml_edit::value(gate.timeout.clone());
    let mut paths = Table::new();
    let mut include = toml_edit::Array::new();
    for pattern in &gate.paths.include {
        include.push(pattern.as_str());
    }
    paths["include"] = Item::Value(TomlValue::Array(include));
    let mut exclude = toml_edit::Array::new();
    for pattern in &gate.paths.exclude {
        exclude.push(pattern.as_str());
    }
    paths["exclude"] = Item::Value(TomlValue::Array(exclude));
    table["paths"] = Item::Table(paths);
    if gate.run_always {
        table["run_always"] = toml_edit::value(true);
    }
    if !gate.depends_on.is_empty() {
        let mut depends = toml_edit::Array::new();
        for item in &gate.depends_on {
            depends.push(item.as_str());
        }
        table["depends_on"] = Item::Value(TomlValue::Array(depends));
    }
    if !gate.profiles.is_empty() {
        let mut profiles = toml_edit::Array::new();
        for item in &gate.profiles {
            profiles.push(item.as_str());
        }
        table["profiles"] = Item::Value(TomlValue::Array(profiles));
    }
    if !gate.branches.is_empty() {
        let mut branches = toml_edit::Array::new();
        for item in &gate.branches {
            branches.push(item.as_str());
        }
        table["branches"] = Item::Value(TomlValue::Array(branches));
    }
    if gate.only_in != "any" {
        table["only_in"] = toml_edit::value(gate.only_in.clone());
    }
    if !gate.os.is_empty() {
        let mut os = toml_edit::Array::new();
        for item in &gate.os {
            os.push(item.as_str());
        }
        table["os"] = Item::Value(TomlValue::Array(os));
    }
    if !gate.tags.is_empty() {
        let mut tags = toml_edit::Array::new();
        for item in &gate.tags {
            tags.push(item.as_str());
        }
        table["tags"] = Item::Value(TomlValue::Array(tags));
    }
    if gate.locked {
        table["locked"] = toml_edit::value(true);
    }
    if gate.remove {
        table["remove"] = toml_edit::value(true);
    }
    Ok(table)
}

fn json_to_toml(value: &JsonValue) -> Result<TomlValue, String> {
    Ok(match value {
        JsonValue::Null => return Err("null values cannot be represented in TOML".to_string()),
        JsonValue::Bool(flag) => TomlValue::Boolean(toml_edit::Formatted::new(*flag)),
        JsonValue::Number(number) => {
            if let Some(integer) = number.as_i64() {
                TomlValue::Integer(toml_edit::Formatted::new(integer))
            } else if let Some(float) = number.as_f64() {
                TomlValue::Float(toml_edit::Formatted::new(float))
            } else {
                return Err("unsupported TOML number".to_string());
            }
        }
        JsonValue::String(text) => TomlValue::String(toml_edit::Formatted::new(text.clone())),
        JsonValue::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(json_to_toml(item)?);
            }
            TomlValue::Array(array)
        }
        JsonValue::Object(map) => {
            let mut inline = InlineTable::new();
            for (key, item) in map {
                inline.insert(key, json_to_toml(item)?);
            }
            TomlValue::InlineTable(inline)
        }
    })
}

fn validate_edited(src: &str) -> Result<(), String> {
    let layer = parse_toml_value(src)?;
    let mut base = builtin_value()?;
    let mut layer = layer;
    if let Some(object) = layer.as_object_mut() {
        object.remove("extends");
    }
    merge_values(&mut base, &layer)?;
    let policy = value_to_policy(base)?;
    let errors: Vec<String> = validate::validate(&policy)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| {
            format!(
                "{}: {}{}",
                diagnostic.code,
                diagnostic.message,
                diagnostic
                    .path
                    .as_ref()
                    .map(|path| format!(" ({path})"))
                    .unwrap_or_default()
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn resolve_insert_index(
    arr: &toml_edit::ArrayOfTables,
    position: &GatePosition,
) -> Result<usize, String> {
    match position {
        GatePosition::Start => Ok(0),
        GatePosition::End => Ok(arr.len()),
        GatePosition::Index(index) => {
            if *index > arr.len() {
                Err(format!("gate index out of range: {index}"))
            } else {
                Ok(*index)
            }
        }
        GatePosition::Before(target) => arr
            .iter()
            .position(|table| table.get("id").and_then(|v| v.as_str()) == Some(target))
            .ok_or_else(|| format!("gate not found: {target}")),
        GatePosition::After(target) => {
            let pos = arr
                .iter()
                .position(|table| table.get("id").and_then(|v| v.as_str()) == Some(target))
                .ok_or_else(|| format!("gate not found: {target}"))?;
            Ok(pos + 1)
        }
    }
}

// Rebind the public helpers to the real implementation by replacing the broken
// function name. Since Rust has no aliasing of fn definitions, redefine
// `resolve_insert_index` correctly here and remove the placeholder above.
// (The placeholder is compiled only if not shadowed; therefore delete it.)
// We instead keep a single correct function named resolve_insert_index.

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::GatePaths;

    fn gate(id: &str) -> Gate {
        Gate {
            id: id.into(),
            name: format!("Gate {id}"),
            description: String::new(),
            uses: "builtin:dco".into(),
            with: JsonValue::Null,
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
    fn set_deep_key() {
        let src = "schema_version = 1\n";
        let out = set_key(src, "workflow.commits.subject_max", EditValue::Integer(100)).unwrap();
        assert!(out.contains("subject_max"));
        assert!(out.contains("100"));
    }

    #[test]
    fn set_and_unset() {
        let src = "schema_version = 1\n";
        let out = set_key(src, "project.name", EditValue::String("demo".into())).unwrap();
        assert!(out.contains("demo"));
        let out2 = unset_key(&out, "project.name").unwrap();
        assert!(!out2.contains("demo"));
    }

    #[test]
    fn preserves_comments() {
        let src = "# header\nschema_version = 1\n";
        let out = set_key(src, "project.name", EditValue::String("demo".into())).unwrap();
        assert!(out.contains("# header"));
    }

    #[test]
    fn set_gate_mode_works() {
        let src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"T\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\nmode = \"block\"\n";
        let out = set_gate_mode(src, "t", Mode::Warn).unwrap();
        assert!(out.contains("warn"));
    }

    #[test]
    fn update_gate_works() {
        let src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"T\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\n";
        let out = update_gate(src, "t", &[("name", EditValue::String("Updated".into()))]).unwrap();
        assert!(out.contains("Updated"));
    }

    #[test]
    fn remove_gate_works() {
        let src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"T\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\n";
        let out = remove_gate(src, "t").unwrap();
        assert!(!out.contains("id = \"t\""));
    }

    #[test]
    fn remove_gate_missing_errors() {
        let src = "schema_version = 1\n";
        assert!(remove_gate(src, "t").is_err());
    }

    #[test]
    fn duplicate_rejects_dup_id() {
        let src = "schema_version = 1\n[[gates]]\nid = \"a\"\nname = \"A\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\n";
        assert!(duplicate_gate(src, "a", "a", GatePosition::End).is_err());
    }

    #[test]
    fn duplicate_and_move_gate() {
        let src = "schema_version = 1\n[[gates]]\nid = \"a\"\nname = \"A\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\n[[gates]]\nid = \"b\"\nname = \"B\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\n";
        let out = duplicate_gate(src, "a", "a-copy", GatePosition::End).unwrap();
        assert!(out.contains("id = \"a-copy\""));
        let out2 = move_gate(&out, "b", GatePosition::Start).unwrap();
        assert!(out2.contains("id = \"b\""));
    }

    #[test]
    fn add_gate_works() {
        let src = "schema_version = 1\n";
        let out = add_gate(src, &gate("new"), GatePosition::End).unwrap();
        assert!(out.contains("id = \"new\""));
    }

    #[test]
    fn enable_disable_gate() {
        let src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"T\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\nmode = \"block\"\n";
        let out = disable_gate(src, "t").unwrap();
        assert!(out.contains("off"));
        let out2 = enable_gate(&out, "t").unwrap();
        assert!(out2.contains("block"));
    }

    #[test]
    fn move_gate_missing_errors() {
        let src = "schema_version = 1\n[[gates]]\nid = \"a\"\nname = \"A\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\n";
        assert!(move_gate(src, "missing", GatePosition::End).is_err());
        assert!(move_gate(src, "a", GatePosition::Before("nope".into())).is_err());
    }

    #[test]
    fn duplicate_missing_source_errors() {
        let src = "schema_version = 1\n";
        assert!(duplicate_gate(src, "a", "b", GatePosition::End).is_err());
    }

    #[test]
    fn set_key_through_id_array() {
        let src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"T\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\nmode = \"block\"\n";
        let out = set_key(src, "gates.t.mode", EditValue::String("warn".into())).unwrap();
        assert!(out.contains("warn"));
    }

    #[test]
    fn update_gate_rejects_invalid() {
        let src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"T\"\nuses = \"builtin:dco\"\nstages = [\"pre-commit\"]\n";
        assert!(update_gate(src, "t", &[("stages", EditValue::Strings(vec![]))]).is_err());
    }
}
