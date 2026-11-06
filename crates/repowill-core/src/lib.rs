// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! RepoWill core behaviour.

pub use repowill_model as model;

pub mod config;
pub mod platform;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(2, 1 + 1);
    }
}
