// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Policy configuration engine.
//!
//! The engine implements the contracts in `plan/03-POLICY-AND-DATA.md`:
//! layered loading and locking (section 2), semantic validation (section 3),
//! weakening detection (section 4), policy hashing (section 5), command
//! trust hashing (section 6), and schema migrations (section 10).
//!
//! Layers are merged as JSON-shaped values before being deserialized into
//! [`repowill_model::Policy`]. Working on the raw value tree is what makes
//! "a later entry replaces only the fields it sets" and the difference
//! between an omitted key and an explicit default value representable.

pub mod editor;
pub mod hash;
pub mod loader;
pub mod migrate;
pub mod strictness;
pub mod validate;
mod value;

pub use editor::*;
pub use hash::*;
pub use loader::*;
pub use migrate::*;
pub use strictness::*;
pub use validate::*;
pub use value::*;

/// Version of the policy engine contract used for `min_repowill_version`
/// checks. RepoWill 1.0 is the first public release; until then the policy
/// engine deliberately reports the 1.0.0 contract so that a schema-v1 policy
/// with the documented default `min_repowill_version = "1.0.0"` is valid.
/// Callers that know the real binary version should use
/// [`validate::validate_for_version`].
pub const ENGINE_VERSION: &str = "1.0.0";
