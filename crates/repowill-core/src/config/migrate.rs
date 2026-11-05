// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use repowill_model::Policy;

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

pub trait Migration {
    fn source_version(&self) -> u32;
    fn target_version(&self) -> u32;
    fn apply(&self, policy: &mut Policy) -> Result<(), String>;
}

pub struct MigrationRunner {
    pub migrations: Vec<Box<dyn Migration>>,
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self { migrations: vec![] }
    }

    pub fn apply_all(&self, policy: &mut Policy) -> Result<(), String> {
        self.apply_all_with_backup(policy, None).map(|_| ())
    }

    pub fn apply_all_with_backup(
        &self,
        policy: &mut Policy,
        backup_dir: Option<&std::path::Path>,
    ) -> Result<String, String> {
        if policy.schema_version > SUPPORTED_SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema version {} (max {})",
                policy.schema_version, SUPPORTED_SCHEMA_VERSION
            ));
        }
        if let Some(dir) = backup_dir {
            let _ = std::fs::create_dir_all(dir);
            let backup_path = dir.join(format!("policy.v{}.bak", policy.schema_version));
            let _ = std::fs::write(&backup_path, toml::to_string(policy).unwrap_or_default());
        }
        let before = toml::to_string(policy).unwrap_or_default();
        let mut current = policy.schema_version;
        while current < SUPPORTED_SCHEMA_VERSION {
            let m = self
                .migrations
                .iter()
                .find(|m| m.source_version() == current);
            match m {
                Some(mig) => {
                    mig.apply(policy)?;
                    current = mig.target_version();
                    policy.schema_version = current;
                }
                None => return Err(format!("no migration from version {}", current)),
            }
        }
        let after = toml::to_string(policy).unwrap_or_default();
        Ok(format!(
            "--- before ---\n{}\n--- after ---\n{}",
            before, after
        ))
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_runner_ok() {
        let mut p = repowill_model::Policy::default();
        MigrationRunner::default().apply_all(&mut p).unwrap();
    }

    #[test]
    fn newer_version_rejected() {
        let mut p = repowill_model::Policy::default();
        #[allow(clippy::field_reassign_with_default)]
        {
            p.schema_version = 99;
        }
        assert!(MigrationRunner::default().apply_all(&mut p).is_err());
    }
}
