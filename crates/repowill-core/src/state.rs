// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Repository state store under `<git-common-dir>/repowill/`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Counters {
    pub next_request: u64,
    pub next_run: u64,
}

impl Default for Counters {
    fn default() -> Self {
        Self {
            next_request: 1,
            next_run: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateStore {
    root: PathBuf,
}

impl StateStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn from_common_dir(common_dir: &Path) -> Self {
        Self::new(common_dir.join("repowill"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn lock_path(&self) -> PathBuf {
        self.root.join("lock")
    }

    pub fn version_path(&self) -> PathBuf {
        self.root.join("VERSION")
    }

    pub fn ensure(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|error| error.to_string())?;
        for name in ["tokens", "requests", "ledger", "runs", "backups"] {
            fs::create_dir_all(self.root.join(name)).map_err(|error| error.to_string())?;
        }
        if !self.version_path().exists() {
            fs::write(self.version_path(), STATE_VERSION.to_string())
                .map_err(|error| error.to_string())?;
        }
        if !self.counters_path().exists() {
            self.write_counters(&Counters::default())?;
        }
        Ok(())
    }

    pub fn read_version(&self) -> Result<u32, String> {
        let text = fs::read_to_string(self.version_path()).map_err(|error| error.to_string())?;
        text.trim()
            .parse::<u32>()
            .map_err(|error| error.to_string())
    }

    pub fn counters_path(&self) -> PathBuf {
        self.root.join("counters.json")
    }

    pub fn read_counters(&self) -> Result<Counters, String> {
        let text = fs::read_to_string(self.counters_path()).map_err(|error| error.to_string())?;
        serde_json::from_str(&text).map_err(|error| error.to_string())
    }

    pub fn write_counters(&self, counters: &Counters) -> Result<(), String> {
        let text = serde_json::to_string_pretty(counters).map_err(|error| error.to_string())?;
        atomic_write(&self.counters_path(), text.as_bytes())
    }

    pub fn next_request_id(&self) -> Result<u64, String> {
        let mut counters = self.read_counters()?;
        let id = counters.next_request;
        counters.next_request += 1;
        self.write_counters(&counters)?;
        Ok(id)
    }

    pub fn next_run_id(&self) -> Result<u64, String> {
        let mut counters = self.read_counters()?;
        let id = counters.next_run;
        counters.next_run += 1;
        self.write_counters(&counters)?;
        Ok(id)
    }

    pub fn audit_path(&self) -> PathBuf {
        self.root.join("audit.jsonl")
    }

    pub fn token_dir(&self) -> PathBuf {
        self.root.join("tokens")
    }

    pub fn create_token(
        &self,
        purpose: &str,
        target: &str,
        expected_old: Option<&str>,
        expected_new: Option<&str>,
        ttl_seconds: u64,
    ) -> Result<String, String> {
        if ttl_seconds == 0 || ttl_seconds > 120 {
            return Err("token TTL must be between 1 and 120 seconds".to_string());
        }
        self.ensure()?;
        let key = KeyStore::new(KeyStore::default_path())
            .load_or_create()
            .map_err(|error| error.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + ttl_seconds;
        let mut token = serde_json::json!({
            "purpose": purpose,
            "target": target,
            "expected_old": expected_old,
            "expected_new": expected_new,
            "expires_at": expires_at,
        });
        let payload = serde_json::to_string(&token).map_err(|error| error.to_string())?;
        let mac = hex_encode(&hmac_sha256(&key, payload.as_bytes()));
        if let Some(map) = token.as_object_mut() {
            map.insert("mac".to_string(), Value::String(mac));
        }
        std::fs::create_dir_all(self.token_dir()).map_err(|error| error.to_string())?;
        atomic_write(
            &self.token_dir().join(format!("{id}.json")),
            serde_json::to_string(&token).unwrap().as_bytes(),
        )?;
        Ok(id)
    }

    pub fn consume_token(&self, token: &str) -> Result<bool, String> {
        self.consume_token_with(token, None, None, None, None)
    }

    pub fn consume_token_with(
        &self,
        token: &str,
        expected_purpose: Option<&str>,
        expected_target: Option<&str>,
        expected_old: Option<&str>,
        expected_new: Option<&str>,
    ) -> Result<bool, String> {
        let path = self.token_dir().join(format!("{token}.json"));
        if !path.exists() {
            return Ok(false);
        }
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = value.get("expires_at").and_then(Value::as_u64).unwrap_or(0);
        let valid = expires_at >= now
            && expected_purpose
                .map(|purpose| value.get("purpose").and_then(Value::as_str) == Some(purpose))
                .unwrap_or(true)
            && expected_target
                .map(|target| value.get("target").and_then(Value::as_str) == Some(target))
                .unwrap_or(true)
            && expected_old
                .map(|old| value.get("expected_old").and_then(Value::as_str) == Some(old))
                .unwrap_or(true)
            && expected_new
                .map(|new| value.get("expected_new").and_then(Value::as_str) == Some(new))
                .unwrap_or(true);
        let _ = std::fs::remove_file(&path);
        Ok(valid)
    }

    pub fn ledger_dir(&self) -> PathBuf {
        self.root.join("ledger")
    }

    pub fn metrics_path(&self) -> PathBuf {
        self.root.join("metrics.jsonl")
    }

    pub fn integrity_path(&self) -> PathBuf {
        self.root.join("integrity.json")
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn ledger_path(&self, tree_oid: &str) -> PathBuf {
        let prefix = tree_oid.get(..2).unwrap_or("00");
        self.root
            .join("ledger")
            .join(prefix)
            .join(format!("{tree_oid}.jsonl"))
    }

    pub fn append_ledger(&self, tree_oid: &str, entry: &Value) -> Result<(), String> {
        let path = self.ledger_path(tree_oid);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let line = serde_json::to_string(entry).map_err(|error| error.to_string())?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        writeln!(file, "{line}").map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn append_metric(&self, entry: &Value) -> Result<(), String> {
        self.ensure()?;
        let line = serde_json::to_string(entry).map_err(|error| error.to_string())?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.metrics_path())
            .map_err(|error| error.to_string())?;
        writeln!(file, "{line}").map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn write_integrity(&self, value: &Value) -> Result<(), String> {
        self.ensure()?;
        atomic_write(
            &self.integrity_path(),
            serde_json::to_string_pretty(value).unwrap().as_bytes(),
        )
    }

    pub fn create_run_dir(&self, run_id: &str) -> Result<PathBuf, String> {
        self.ensure()?;
        let dir = self.runs_dir().join(run_id);
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        Ok(dir)
    }

    pub fn append_audit(&self, key: &[u8], entry: &Value) -> Result<(), String> {
        let mut chain = String::new();
        if self.audit_path().exists() {
            chain = fs::read_to_string(self.audit_path()).map_err(|error| error.to_string())?;
        }
        let prev_hash = last_audit_hash(&chain);
        let seq = chain.lines().filter(|line| !line.trim().is_empty()).count() as u64 + 1;
        let at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        let mut object = entry.clone();
        if let Some(map) = object.as_object_mut() {
            map.entry("seq".to_string()).or_insert_with(|| json!(seq));
            map.entry("at".to_string())
                .or_insert_with(|| Value::String(at.clone()));
            map.entry("actor")
                .or_insert_with(|| json!({"kind":"system","agent":null,"os_user":null}));
            map.entry("event")
                .or_insert_with(|| Value::String("audit.entry".to_string()));
            map.entry("subject").or_insert_with(|| json!({}));
            map.entry("details").or_insert_with(|| json!({}));
            map.insert("prev".to_string(), Value::String(prev_hash.clone()));
        }
        let payload = serde_json::to_string(&object).map_err(|error| error.to_string())?;
        let entry_hash = sha256_hex(payload.as_bytes());
        let mac = hmac_sha256(key, entry_hash.as_bytes());
        let mac_hex = hex_encode(&mac);
        let mut final_object = object;
        if let Some(map) = final_object.as_object_mut() {
            map.insert("hash".to_string(), Value::String(entry_hash));
            map.insert("mac".to_string(), Value::String(mac_hex));
        }
        let line = serde_json::to_string(&final_object).map_err(|error| error.to_string())?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.audit_path())
            .map_err(|error| error.to_string())?;
        use std::io::Write;
        writeln!(file, "{line}").map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn verify_audit(&self, key: &[u8]) -> Result<bool, String> {
        Ok(self.verify_audit_index(key)?.is_none())
    }

    pub fn verify_audit_index(&self, key: &[u8]) -> Result<Option<u64>, String> {
        if !self.audit_path().exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(self.audit_path()).map_err(|error| error.to_string())?;
        let mut prev = "GENESIS".to_string();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
            let seq = value.get("seq").and_then(Value::as_u64).unwrap_or(0);
            let entry_hash = value.get("hash").and_then(Value::as_str).unwrap_or("");
            let mac = value.get("mac").and_then(Value::as_str).unwrap_or("");
            let prev_hash = value.get("prev").and_then(Value::as_str).unwrap_or("");
            if prev_hash != prev {
                return Ok(Some(seq));
            }
            let mut object = value.clone();
            if let Some(map) = object.as_object_mut() {
                map.remove("hash");
                map.remove("mac");
            }
            let payload = serde_json::to_string(&object).map_err(|error| error.to_string())?;
            let expected_hash = sha256_hex(payload.as_bytes());
            if expected_hash != entry_hash {
                return Ok(Some(seq));
            }
            let expected_mac = hex_encode(&hmac_sha256(key, entry_hash.as_bytes()));
            if expected_mac != mac {
                return Ok(Some(seq));
            }
            prev = entry_hash.to_string();
        }
        Ok(None)
    }
}

fn last_audit_hash(chain: &str) -> String {
    let mut last = "GENESIS".to_string();
    for line in chain.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(line)
            && let Some(hash) = value.get("hash").and_then(Value::as_str)
        {
            last = hash.to_string();
        }
    }
    last
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::platform::fs::atomic_write(path, bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key = key.to_vec();
    if key.len() > BLOCK_SIZE {
        let mut hasher = Sha256::new();
        hasher.update(&key);
        key = hasher.finalize().to_vec();
    }
    if key.len() < BLOCK_SIZE {
        key.resize(BLOCK_SIZE, 0);
    }
    let mut o_key_pad = vec![0x5c; BLOCK_SIZE];
    let mut i_key_pad = vec![0x36; BLOCK_SIZE];
    for (index, byte) in key.iter().enumerate() {
        o_key_pad[index] ^= byte;
        i_key_pad[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(&i_key_pad);
    inner.update(message);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(&o_key_pad);
    outer.update(inner_hash);
    outer.finalize().into()
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

#[derive(Debug, Clone)]
pub struct KeyStore {
    path: PathBuf,
}

impl KeyStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> PathBuf {
        if let Ok(home) = std::env::var("REPOWILL_HOME") {
            return PathBuf::from(home).join("keys").join("approval.key");
        }
        if let Some(dirs) = directories::ProjectDirs::from("org", "AfterLeads", "RepoWill") {
            return dirs.data_dir().join("keys").join("approval.key");
        }
        PathBuf::from(".repowill").join("approval.key")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_create(&self) -> Result<Vec<u8>, String> {
        if self.path.exists() {
            return std::fs::read(&self.path).map_err(|error| error.to_string());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|error| error.to_string())?;
        std::fs::write(&self.path, key).map_err(|error| error.to_string())?;
        let _ = crate::platform::fs::set_restrictive_permissions(&self.path);
        Ok(key.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn state_store_round_trip() {
        let dir = tempdir().unwrap();
        let store = StateStore::from_common_dir(dir.path());
        store.ensure().unwrap();
        assert_eq!(store.read_version().unwrap(), STATE_VERSION);
        let id = store.next_request_id().unwrap();
        assert_eq!(id, 1);
        assert_eq!(store.read_counters().unwrap().next_request, 2);
    }

    #[test]
    fn audit_chain_verifies() {
        let dir = tempdir().unwrap();
        let store = StateStore::from_common_dir(dir.path());
        store.ensure().unwrap();
        let key = b"secret";
        store
            .append_audit(key, &serde_json::json!({ "op": "test", "result": "ok" }))
            .unwrap();
        assert!(store.verify_audit(key).unwrap());
    }
}
