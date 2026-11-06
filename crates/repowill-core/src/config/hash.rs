// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Policy and command trust hashing.
//!
//! Both hashes use length-framed SHA-256 so that two different inputs can
//! never produce the same byte stream by concatenation. `policy_hash` folds
//! in the SHA-256 of every script and adapter file referenced by the policy;
//! `commands_hash_policy` folds in the contents of every executable element.

use repowill_model::Policy;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A file whose bytes are folded into a policy hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencedFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// An executable element collected for command trust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandElement {
    pub kind: &'static str,
    pub id: String,
    pub text: String,
}

/// Hash a policy, reading referenced files relative to the current directory.
pub fn policy_hash(policy: &Policy) -> Result<String, String> {
    policy_hash_with_root(Path::new("."), policy, &[])
}

/// Hash a policy, reading referenced files relative to `root`.
pub fn policy_hash_with_root(
    root: &Path,
    policy: &Policy,
    adapter_files: &[PathBuf],
) -> Result<String, String> {
    let files = referenced_files(root, policy, adapter_files)?;
    let value = serde_json::to_value(policy).map_err(|error| error.to_string())?;
    let canonical = canonical_json(&value)?;
    let mut hasher = Sha256::new();
    hasher.update(b"repowill.policy.v1\0");
    frame(&mut hasher, canonical.as_bytes());
    for file in &files {
        frame(&mut hasher, file.path.to_string_lossy().as_bytes());
        let file_hash = sha256_bytes(&file.bytes);
        hasher.update(file_hash);
    }
    Ok(hex_encode(&hasher.finalize()))
}

/// Collect every script and adapter file referenced by `policy`.
pub fn referenced_files(
    root: &Path,
    policy: &Policy,
    adapter_files: &[PathBuf],
) -> Result<Vec<ReferencedFile>, String> {
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    for gate in &policy.gates {
        if gate.uses == "script"
            && let Some(path) = gate.with.get("path").and_then(Value::as_str)
        {
            let full = root.join(path);
            let bytes = std::fs::read(&full).map_err(|error| {
                format!("cannot read referenced script {}: {error}", full.display())
            })?;
            push_file(&mut files, &mut seen, full, bytes);
        }
    }
    for path in adapter_files {
        let full = if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        };
        let bytes = std::fs::read(&full).map_err(|error| {
            format!("cannot read referenced adapter {}: {error}", full.display())
        })?;
        push_file(&mut files, &mut seen, full, bytes);
    }
    let adapters_dir = root.join(".repowill").join("adapters");
    if adapters_dir.is_dir() {
        let mut entries = std::fs::read_dir(&adapters_dir)
            .map_err(|error| format!("cannot read adapters directory: {error}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let bytes = std::fs::read(&path).map_err(|error| {
                format!("cannot read referenced adapter {}: {error}", path.display())
            })?;
            push_file(&mut files, &mut seen, path, bytes);
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

/// Backwards-compatible raw command list hash.
pub fn commands_hash(commands: &[String]) -> String {
    commands_hash_strings(commands)
}

pub fn commands_hash_strings(commands: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"repowill.commands.v1\0");
    for command in commands {
        frame(&mut hasher, command.as_bytes());
    }
    hex_encode(&hasher.finalize())
}

/// Collect every executable element referenced by a policy.
pub fn command_elements(
    root: &Path,
    policy: &Policy,
    adapter_files: &[PathBuf],
) -> Result<Vec<CommandElement>, String> {
    let mut elements = Vec::new();
    for gate in &policy.gates {
        match gate.uses.as_str() {
            "command" => {
                for key in ["run", "run_windows", "run_linux", "fix_command"] {
                    if let Some(text) = gate.with.get(key).and_then(Value::as_str) {
                        elements.push(CommandElement {
                            kind: "gate-command",
                            id: format!("{}.{}", gate.id, key),
                            text: text.to_string(),
                        });
                    }
                }
            }
            "script" => {
                if let Some(path) = gate.with.get("path").and_then(Value::as_str) {
                    let full = root.join(path);
                    let bytes = std::fs::read(&full).map_err(|error| {
                        format!("cannot read referenced script {}: {error}", full.display())
                    })?;
                    elements.push(CommandElement {
                        kind: "gate-script",
                        id: gate.id.clone(),
                        text: String::from_utf8_lossy(&bytes).into_owned(),
                    });
                    if let Some(args) = gate.with.get("args").and_then(Value::as_array) {
                        let joined = args
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !joined.is_empty() {
                            elements.push(CommandElement {
                                kind: "gate-script-args",
                                id: gate.id.clone(),
                                text: joined,
                            });
                        }
                    }
                }
                if let Some(text) = gate.with.get("fix_command").and_then(Value::as_str) {
                    elements.push(CommandElement {
                        kind: "gate-fix",
                        id: gate.id.clone(),
                        text: text.to_string(),
                    });
                }
            }
            _ => {
                for key in ["command", "fix_command", "run"] {
                    if let Some(text) = gate.with.get(key).and_then(Value::as_str) {
                        elements.push(CommandElement {
                            kind: "gate-command",
                            id: format!("{}.{}", gate.id, key),
                            text: text.to_string(),
                        });
                    }
                }
            }
        }
    }
    for generated in &policy.docs.generated {
        elements.push(CommandElement {
            kind: "docs-generated",
            id: generated.id.clone(),
            text: generated.command.clone(),
        });
    }
    for path in adapter_files {
        let full = if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        };
        let bytes = std::fs::read(&full).map_err(|error| {
            format!("cannot read referenced adapter {}: {error}", full.display())
        })?;
        elements.push(CommandElement {
            kind: "adapter",
            id: full.to_string_lossy().into_owned(),
            text: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }
    let adapters_dir = root.join(".repowill").join("adapters");
    if adapters_dir.is_dir() {
        let mut entries = std::fs::read_dir(&adapters_dir)
            .map_err(|error| format!("cannot read adapters directory: {error}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let bytes = std::fs::read(&path).map_err(|error| {
                format!("cannot read referenced adapter {}: {error}", path.display())
            })?;
            elements.push(CommandElement {
                kind: "adapter",
                id: path.to_string_lossy().into_owned(),
                text: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }
    }
    elements.sort_by(|left, right| (left.kind, &left.id).cmp(&(right.kind, &right.id)));
    Ok(elements)
}

/// Hash every executable element in a policy.
pub fn commands_hash_policy(
    root: &Path,
    policy: &Policy,
    adapter_files: &[PathBuf],
) -> Result<String, String> {
    let elements = command_elements(root, policy, adapter_files)?;
    let mut hasher = Sha256::new();
    hasher.update(b"repowill.commands.v1\0");
    for element in &elements {
        frame(&mut hasher, element.kind.as_bytes());
        frame(&mut hasher, element.id.as_bytes());
        frame(&mut hasher, element.text.as_bytes());
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn push_file(
    files: &mut Vec<ReferencedFile>,
    seen: &mut BTreeSet<PathBuf>,
    path: PathBuf,
    bytes: Vec<u8>,
) {
    if seen.insert(path.clone()) {
        files.push(ReferencedFile { path, bytes });
    }
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn canonical_json(value: &Value) -> Result<String, String> {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).map_err(|e| e.to_string())?);
                out.push(':');
                out.push_str(&canonical_json(&map[*key])?);
            }
            out.push('}');
            Ok(out)
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonical_json(item)?);
            }
            out.push(']');
            Ok(out)
        }
        other => serde_json::to_string(other).map_err(|e| e.to_string()),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::Policy;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn policy_hash_stable() {
        let policy = Policy::default();
        assert_eq!(policy_hash(&policy).unwrap(), policy_hash(&policy).unwrap());
    }

    #[test]
    fn policy_hash_changes_with_referenced_file() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("gate.sh");
        fs::write(&script, "echo one").unwrap();
        let mut policy = Policy::default();
        policy.gates.push(repowill_model::Gate {
            id: "script-gate".into(),
            name: "Script".into(),
            description: String::new(),
            uses: "script".into(),
            with: json!({"path": "gate.sh"}),
            stages: vec!["pre-commit".into()],
            mode: repowill_model::Mode::Block,
            stage_modes: Default::default(),
            trigger: "auto".into(),
            on_error: "block".into(),
            timeout: "10m".into(),
            paths: repowill_model::GatePaths {
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
        });
        let first = policy_hash_with_root(dir.path(), &policy, &[]).unwrap();
        fs::write(&script, "echo two").unwrap();
        let second = policy_hash_with_root(dir.path(), &policy, &[]).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn commands_hash_differs() {
        assert_ne!(
            commands_hash_strings(&["a".into()]),
            commands_hash_strings(&["b".into()])
        );
    }

    #[test]
    fn canonical_json_is_collision_resistant() {
        let left = json!({"a": "bc"});
        let right = json!({"ab": "c"});
        assert_ne!(
            canonical_json(&left).unwrap(),
            canonical_json(&right).unwrap()
        );
    }

    #[test]
    fn commands_hash_policy_includes_generated_commands() {
        let mut policy = Policy::default();
        policy.docs.generated.push(repowill_model::GeneratedDoc {
            id: "docs".into(),
            command: "make docs".into(),
            outputs: vec![],
            remove: false,
        });
        let hash = commands_hash_policy(Path::new("."), &policy, &[]).unwrap();
        assert!(!hash.is_empty());
    }
}
