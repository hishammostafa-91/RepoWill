// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use sha2::{Digest, Sha256};

pub fn policy_hash(policy: &repowill_model::Policy) -> Result<String, String> {
    let val = serde_json::to_value(policy).map_err(|e| e.to_string())?;
    let canon = canonical_json(&val);
    let mut h = Sha256::new();
    h.update(canon.as_bytes());
    // Fold in referenced script/adapter files when present.
    for g in &policy.gates {
        if let Some(obj) = g.with.as_object() {
            for key in ["script", "path", "file"] {
                if let Some(v) = obj.get(key).and_then(|x| x.as_str())
                    && let Ok(bytes) = std::fs::read(v)
                {
                    let mut fh = Sha256::new();
                    fh.update(&bytes);
                    h.update(hex_encode(&fh.finalize()).as_bytes());
                }
            }
        }
    }
    Ok(hex_encode(&h.finalize()))
}

pub fn commands_hash(commands: &[String]) -> String {
    let mut h = Sha256::new();
    for c in commands {
        h.update(c.as_bytes());
        h.update(b"\n");
    }
    hex_encode(&h.finalize())
}

fn canonical_json(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap_or_default());
                out.push(':');
                out.push_str(&canonical_json(&map[*k]));
            }
            out.push('}');
            out
        }
        serde_json::Value::Array(arr) => {
            let mut out = String::from("[");
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&canonical_json(v));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::Policy;

    #[test]
    fn policy_hash_stable() {
        let p = Policy::default();
        assert_eq!(policy_hash(&p).unwrap(), policy_hash(&p).unwrap());
    }

    #[test]
    fn commands_hash_differs() {
        assert_ne!(commands_hash(&["a".into()]), commands_hash(&["b".into()]));
    }
}
