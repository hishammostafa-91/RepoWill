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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get_secret_callable() {
        let _ = get_secret("repowill", "test-acct");
    }
}
