// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! RepoWill shared types and schemas.

pub mod ops;
pub mod policy;

pub use ops::*;
pub use policy::*;

#[cfg(test)]
mod tests {
    use super::policy::Policy;
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
    }
}
