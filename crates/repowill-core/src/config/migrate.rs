// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use crate::config::value::policy_to_toml;
use repowill_model::Policy;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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

    pub fn with(migrations: Vec<Box<dyn Migration>>) -> Self {
        Self { migrations }
    }

    pub fn apply_all(&self, policy: &mut Policy) -> Result<(), String> {
        self.apply_all_with_backup(policy, None).map(|_| ())
    }

    pub fn apply_all_with_backup(
        &self,
        policy: &mut Policy,
        backup_dir: Option<&Path>,
    ) -> Result<String, String> {
        self.apply_all_outcome(policy, backup_dir)
            .map(|outcome| outcome.diff())
    }

    pub fn apply_all_outcome(
        &self,
        policy: &mut Policy,
        backup_dir: Option<&Path>,
    ) -> Result<MigrationOutcome, String> {
        if policy.schema_version > SUPPORTED_SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema version {} (max {})",
                policy.schema_version, SUPPORTED_SCHEMA_VERSION
            ));
        }
        let chain = migration_chain(
            &self.migrations,
            policy.schema_version,
            SUPPORTED_SCHEMA_VERSION,
        )?;
        let from_version = policy.schema_version;
        let before = policy_to_toml(policy)?;

        let backup = if let Some(dir) = backup_dir {
            std::fs::create_dir_all(dir).map_err(|error| {
                format!("cannot create backup directory {}: {error}", dir.display())
            })?;
            let backup_path = dir.join(format!("policy.v{}.toml", policy.schema_version));
            std::fs::write(&backup_path, &before).map_err(|error| {
                format!("cannot write backup {}: {error}", backup_path.display())
            })?;
            Some(backup_path)
        } else {
            None
        };

        let mut trial = policy.clone();
        for index in chain {
            let migration = &self.migrations[index];
            migration
                .apply(&mut trial)
                .map_err(|error| error.to_string())?;
            if trial.schema_version != migration.target_version() {
                trial.schema_version = migration.target_version();
            }
        }
        trial.schema_version = SUPPORTED_SCHEMA_VERSION;
        let after = policy_to_toml(&trial).map_err(|error| error.to_string())?;
        *policy = trial;

        Ok(MigrationOutcome {
            from_version,
            to_version: SUPPORTED_SCHEMA_VERSION,
            backup,
            before,
            after,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MigrationOutcome {
    pub from_version: u32,
    pub to_version: u32,
    pub backup: Option<PathBuf>,
    pub before: String,
    pub after: String,
}

impl MigrationOutcome {
    pub fn diff(&self) -> String {
        format!(
            "--- before ---\n{}\n--- after ---\n{}",
            self.before, self.after
        )
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::new()
    }
}

fn migration_chain(
    migrations: &[Box<dyn Migration>],
    start: u32,
    target: u32,
) -> Result<Vec<usize>, String> {
    let mut current = start;
    let mut visited = BTreeSet::new();
    let mut indices = Vec::new();
    while current < target {
        if !visited.insert(current) {
            return Err(format!("migration cycle detected at version {current}"));
        }
        let matches: Vec<(usize, &Box<dyn Migration>)> = migrations
            .iter()
            .enumerate()
            .filter(|(_, migration)| migration.source_version() == current)
            .collect();
        if matches.len() > 1 {
            return Err(format!("multiple migrations from version {current}"));
        }
        let (index, migration) = matches
            .into_iter()
            .next()
            .ok_or_else(|| format!("no migration from version {current}"))?;
        if migration.target_version() <= migration.source_version() {
            return Err(format!(
                "migration from {} has invalid target {}",
                migration.source_version(),
                migration.target_version()
            ));
        }
        if migration.target_version() != current + 1 {
            return Err(format!(
                "migration from {} targets {} instead of {}",
                current,
                migration.target_version(),
                current + 1
            ));
        }
        indices.push(index);
        current = migration.target_version();
    }
    if current != target {
        return Err(format!(
            "migration stopped at version {current}, expected {target}"
        ));
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::Policy;

    struct BumpVersion;

    impl Migration for BumpVersion {
        fn source_version(&self) -> u32 {
            0
        }

        fn target_version(&self) -> u32 {
            1
        }

        fn apply(&self, policy: &mut Policy) -> Result<(), String> {
            policy.min_repowill_version = "1.0.0".to_string();
            Ok(())
        }
    }

    #[test]
    fn default_runner_ok() {
        let mut policy = Policy::default();
        MigrationRunner::default().apply_all(&mut policy).unwrap();
    }

    #[test]
    fn newer_version_rejected() {
        let mut policy = Policy {
            schema_version: 99,
            ..Policy::default()
        };
        assert!(MigrationRunner::default().apply_all(&mut policy).is_err());
    }

    #[test]
    fn chain_validation_rejects_gap() {
        struct Gap;
        impl Migration for Gap {
            fn source_version(&self) -> u32 {
                0
            }
            fn target_version(&self) -> u32 {
                2
            }
            fn apply(&self, _policy: &mut Policy) -> Result<(), String> {
                Ok(())
            }
        }
        let runner = MigrationRunner::with(vec![Box::new(Gap)]);
        let mut policy = Policy {
            schema_version: 0,
            ..Policy::default()
        };
        assert!(runner.apply_all(&mut policy).is_err());
    }

    #[test]
    fn backup_error_propagates() {
        let runner = MigrationRunner::with(vec![Box::new(BumpVersion)]);
        let mut policy = Policy {
            schema_version: 0,
            ..Policy::default()
        };
        let file = tempfile::NamedTempFile::new().unwrap();
        let result = runner.apply_all_with_backup(&mut policy, Some(file.path()));
        assert!(result.is_err());
    }

    #[test]
    fn migration_writes_backup_and_changes_version() {
        let runner = MigrationRunner::with(vec![Box::new(BumpVersion)]);
        let mut policy = Policy {
            schema_version: 0,
            ..Policy::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let outcome = runner
            .apply_all_outcome(&mut policy, Some(dir.path()))
            .unwrap();
        assert_eq!(policy.schema_version, 1);
        assert!(outcome.backup.unwrap().exists());
        assert!(outcome.before.contains("schema_version = 0"));
        assert!(outcome.after.contains("schema_version = 1"));
    }

    #[test]
    fn golden_migration_v0_to_v1() {
        let runner = MigrationRunner::with(vec![Box::new(BumpVersion)]);
        let mut policy = Policy {
            schema_version: 0,
            ..Policy::default()
        };
        let outcome = runner.apply_all_outcome(&mut policy, None).unwrap();
        let expected = include_str!("../../tests/fixtures/config/migrate/v0-expected.toml");
        assert_eq!(outcome.after.trim(), expected.trim());
    }

    #[test]
    fn null_safe_toml_serialization() {
        let policy = Policy::default();
        let text = policy_to_toml(&policy).unwrap();
        assert!(text.contains("schema_version"));
        assert!(!text.contains("null"));
    }
}
