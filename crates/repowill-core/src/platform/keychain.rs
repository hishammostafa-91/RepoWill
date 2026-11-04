// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::path::PathBuf;

pub enum SecretStore {
    Keyring,
    EnvFallback,
    FileFallback(PathBuf),
}

pub fn get_secret(service: &str, account: &str) -> Result<Option<String>, String> {
    // Try keyring first.
    match keyring::Entry::new(service, account) {
        Ok(entry) => match entry.get_password() {
            Ok(p) => return Ok(Some(p)),
            Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(e.to_string()),
        },
        Err(e) => return Err(e.to_string()),
    }
    // Env fallback.
    let key = format!(
        "{}_{}",
        service.to_uppercase().replace('-', "_"),
        account.to_uppercase().replace('-', "_")
    );
    if let Ok(v) = std::env::var(&key) {
        return Ok(Some(v));
    }
    Ok(None)
}

pub fn set_secret(service: &str, account: &str, value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(service, account).map_err(|e| e.to_string())?;
    entry.set_password(value).map_err(|e| e.to_string())
}

pub fn get_secret_file(path: &std::path::Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let v = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(Some(v))
}

pub fn set_secret_file(path: &std::path::Path, value: &str) -> Result<(), String> {
    super::fs::atomic_write(path, value.as_bytes())?;
    super::fs::set_restrictive_permissions(path)?;
    Ok(())
}

impl SecretStore {
    pub fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        match self {
            SecretStore::Keyring => get_secret(service, account),
            SecretStore::EnvFallback => {
                let key = format!(
                    "{}_{}",
                    service.to_uppercase().replace('-', "_"),
                    account.to_uppercase().replace('-', "_")
                );
                Ok(std::env::var(&key).ok())
            }
            SecretStore::FileFallback(p) => get_secret_file(p),
        }
    }
    pub fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        match self {
            SecretStore::Keyring => set_secret(service, account, value),
            SecretStore::EnvFallback => {
                // Environment fallback is read-only; cannot set at runtime.
                Err("env fallback is read-only".to_string())
            }
            SecretStore::FileFallback(p) => set_secret_file(p, value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get_secret_callable() {
        let _ = get_secret("repowill", "test-acct");
    }
}
