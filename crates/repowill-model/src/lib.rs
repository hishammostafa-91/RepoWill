// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! RepoWill shared types and schemas.

pub mod ops;
pub mod policy;

pub use ops::*;
pub use policy::*;

#[cfg(test)]
mod tests {
    use super::policy::{Mode, Policy};
    use toml::from_str;

    #[test]
    fn minimal_policy_parses() {
        let src = "schema_version = 1\n";
        let p: Policy = from_str(src).unwrap();
        assert_eq!(p.schema_version, 1);
        assert_eq!(p.project.preset, "recommended");
    }

    #[test]
    fn recommended_defaults_roundtrip() {
        let p = Policy::default();
        let s = toml::to_string(&p).unwrap();
        let p2: Policy = toml::from_str(&s).unwrap();
        assert_eq!(p2.schema_version, p.schema_version);
        assert_eq!(p2.lock.keys, p.lock.keys);
    }

    #[test]
    fn lock_keys_roundtrip() {
        let src = "schema_version = 1\n[lock]\nkeys = [\"dependencies.licenses\", \"gates\"]\n";
        let p: Policy = toml::from_str(src).unwrap();
        assert_eq!(
            p.lock.keys,
            vec!["dependencies.licenses".to_string(), "gates".to_string()]
        );
        let s = toml::to_string(&p).unwrap();
        let p2: Policy = toml::from_str(&s).unwrap();
        assert_eq!(p2.lock.keys, p.lock.keys);
    }

    #[test]
    fn policy_with_gate_parses() {
        let src = r#"
schema_version = 1
[[gates]]
id = "tests"
name = "Tests"
uses = "builtin:tests"
stages = ["pre-commit"]
"#;
        let p: Policy = toml::from_str(src).unwrap();
        assert_eq!(p.gates.len(), 1);
        assert_eq!(p.gates[0].id, "tests");
        assert_eq!(p.gates[0].mode, Mode::Block);
    }

    #[test]
    fn registry_has_all_ops() {
        use crate::ops::{all_operations, registry};
        assert_eq!(registry().len(), all_operations().len());
        assert!(registry().iter().all(|m| !m.cli_path.is_empty()));
    }

    #[test]
    fn enum_roundtrip() {
        use crate::ops::Operation;
        let o = Operation::MergeRun;
        let s = serde_json::to_string(&o).unwrap();
        let o2: Operation = serde_json::from_str(&s).unwrap();
        assert_eq!(o, o2);
    }
}
