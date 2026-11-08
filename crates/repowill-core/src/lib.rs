// SPDX-License-Identifier: AGPL-3.0-or-later
#![deny(unsafe_code)]
//! RepoWill core behaviour.

pub use repowill_model as model;
pub use repowill_model::Operation;

pub mod config;
pub mod git;
pub mod ops;
pub mod platform;
pub mod project;
pub mod state;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(2, 1 + 1);
    }
}
