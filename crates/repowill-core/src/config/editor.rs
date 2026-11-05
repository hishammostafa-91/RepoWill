// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use toml_edit::{DocumentMut, Item, Table};

pub fn set_key(src: &str, key: &str, value: &str) -> Result<String, String> {
    let mut doc: DocumentMut = src
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    let parts: Vec<&str> = key.split('.').collect();
    // walk/create intermediate tables
    // Operate on the document's root table.
    set_nested(doc.as_table_mut(), &parts, value)?;
    Ok(doc.to_string())
}

fn set_nested(table: &mut Table, parts: &[&str], value: &str) -> Result<(), String> {
    if parts.is_empty() {
        return Ok(());
    }
    if parts.len() == 1 {
        match value.parse::<Item>() {
            Ok(item) => table.insert(parts[0], item),
            Err(_) => table.insert(parts[0], toml_edit::value(value)),
        };
        return Ok(());
    }
    if !table.contains_key(parts[0]) {
        table.insert(parts[0], Item::Table(Table::new()));
    }
    if let Some(item) = table.get_mut(parts[0])
        && let Some(t) = item.as_table_mut()
    {
        return set_nested(t, &parts[1..], value);
    }
    Err("not a table".into())
}

pub fn unset_key(src: &str, key: &str) -> Result<String, String> {
    let mut doc: DocumentMut = src
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    let parts: Vec<&str> = key.split('.').collect();
    unset_nested(doc.as_table_mut(), &parts);
    Ok(doc.to_string())
}

fn unset_nested(table: &mut Table, parts: &[&str]) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        table.remove(parts[0]);
        return;
    }
    if let Some(item) = table.get_mut(parts[0])
        && let Some(t) = item.as_table_mut()
    {
        unset_nested(t, &parts[1..]);
    }
}

pub fn add_gate(src: &str, gate_toml: &str) -> Result<String, String> {
    // Append a [[gates]] block.
    let mut out = src.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(gate_toml);
    if !gate_toml.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

pub fn move_gate(src: &str, id: &str, before_id: &str) -> Result<String, String> {
    let mut doc: DocumentMut = src
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    if let Some(item) = doc.get_mut("gates")
        && let Some(arr) = item.as_array_of_tables_mut()
    {
        // find and remove the gate
        let mut gate: Option<toml_edit::Table> = None;
        let mut idx = None;
        for (i, t) in arr.iter().enumerate() {
            if t.get("id").and_then(|v| v.as_str()) == Some(id) {
                gate = Some(t.clone());
                idx = Some(i);
                break;
            }
        }
        if let (Some(g), Some(i)) = (gate, idx) {
            arr.remove(i);
            // find new position
            let mut new_idx = arr.len();
            for (j, t) in arr.iter().enumerate() {
                if t.get("id").and_then(|v| v.as_str()) == Some(before_id) {
                    new_idx = j;
                    break;
                }
            }
            arr.insert(new_idx, g);
        }
    }
    Ok(doc.to_string())
}

pub fn update_gate(src: &str, id: &str, updates: &[(&str, &str)]) -> Result<String, String> {
    let mut doc: DocumentMut = src
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    if let Some(item) = doc.get_mut("gates")
        && let Some(arr) = item.as_array_of_tables_mut()
    {
        for t in arr.iter_mut() {
            if t.get("id").and_then(|v| v.as_str()) == Some(id) {
                for (k, v) in updates {
                    match v.parse::<toml_edit::Item>() {
                        Ok(item) => {
                            t[*k] = item;
                        }
                        Err(_) => {
                            t[*k] = toml_edit::value(*v);
                        }
                    }
                }
            }
        }
    }
    Ok(doc.to_string())
}

pub fn duplicate_gate(src: &str, id: &str, new_id: &str) -> Result<String, String> {
    let mut doc: DocumentMut = src
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    if let Some(item) = doc.get("gates")
        && let Some(arr) = item.as_array_of_tables()
    {
        let mut found: Option<toml_edit::Table> = None;
        for t in arr.iter() {
            if t.get("id").and_then(|v| v.as_str()) == Some(id) {
                found = Some(t.clone());
                break;
            }
        }
        if let Some(mut tbl) = found {
            tbl["id"] = toml_edit::value(new_id);
            if let Some(arr) = doc
                .get_mut("gates")
                .and_then(|i| i.as_array_of_tables_mut())
            {
                arr.push(tbl);
            }
        }
    }
    Ok(doc.to_string())
}

pub fn set_gate_mode(src: &str, id: &str, mode: &str) -> Result<String, String> {
    let mut doc: DocumentMut = src
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    if let Some(item) = doc.get_mut("gates")
        && let Some(arr) = item.as_array_of_tables_mut()
    {
        for t in arr.iter_mut() {
            if t.get("id").and_then(|v| v.as_str()) == Some(id) {
                t["mode"] = toml_edit::value(mode);
            }
        }
    }
    Ok(doc.to_string())
}

pub fn remove_gate(src: &str, id: &str) -> Result<String, String> {
    let mut doc: DocumentMut = src
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    if let Some(item) = doc.get_mut("gates")
        && let Some(arr) = item.as_array_of_tables_mut()
    {
        // find index
        let mut idx = None;
        for (i, t) in arr.iter().enumerate() {
            if t.get("id").and_then(|v| v.as_str()) == Some(id) {
                idx = Some(i);
                break;
            }
        }
        if let Some(i) = idx {
            arr.remove(i);
        }
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_deep_key() {
        let src = "schema_version = 1\n";
        let out = set_key(src, "workflow.commits.subject_max", "100").unwrap();
        assert!(out.contains("subject_max"));
        assert!(out.contains("100"));
    }

    #[test]
    fn set_and_unset() {
        let src = "schema_version = 1\n";
        let out = set_key(src, "project.name", "\"demo\"").unwrap();
        assert!(out.contains("demo"));
        let out2 = unset_key(&out, "project.name").unwrap();
        assert!(!out2.contains("demo"));
    }

    #[test]
    fn preserves_comments() {
        let src = "# header\nschema_version = 1\n";
        let out = set_key(src, "project.name", "\"demo\"").unwrap();
        assert!(out.contains("# header"));
    }

    #[test]
    fn remove_gate_works() {
        let src = "schema_version = 1\n[[gates]]\nid = \"t\"\nname = \"T\"\nuses = \"builtin:t\"\nstages = [\"pre-commit\"]\n";
        let out = remove_gate(src, "t").unwrap();
        assert!(!out.contains("id = \"t\""));
    }
}
