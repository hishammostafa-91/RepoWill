// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use repowill_model::{Mode, Policy};

pub fn mode_tightened(old: Mode, new: Mode) -> bool {
    rank(new) >= rank(old)
}

fn rank(m: Mode) -> u8 {
    match m {
        Mode::Off => 0,
        Mode::Warn => 1,
        Mode::Block => 2,
    }
}

/// Data-driven: every guard mode in `new` must be >= the corresponding mode in `old`.
pub fn local_tighten_only(base: &Policy, local: &Policy) -> bool {
    mode_tightened(base.guards.main_protection, local.guards.main_protection)
        && mode_tightened(base.guards.branch_origin, local.guards.branch_origin)
        && mode_tightened(base.guards.branch_naming, local.guards.branch_naming)
        && mode_tightened(
            base.guards.history_protection,
            local.guards.history_protection,
        )
        && mode_tightened(base.guards.delete_guard, local.guards.delete_guard)
        && mode_tightened(base.guards.protected_paths, local.guards.protected_paths)
        && mode_tightened(base.guards.policy_guard, local.guards.policy_guard)
        && mode_tightened(base.guards.identity, local.guards.identity)
        && mode_tightened(base.guards.merge_guard, local.guards.merge_guard)
        && mode_tightened(base.guards.push_guard, local.guards.push_guard)
        && mode_tightened(base.guards.agent_guard, local.guards.agent_guard)
        && local.gates.len() >= base.gates.len()
        && local.lock.keys.len() >= base.lock.keys.len()
        && local.protection.protected_paths.len() >= base.protection.protected_paths.len()
        && local.waivers.len() >= base.waivers.len()
}

/// Semantic diff: list all detected changes between two policies.
pub fn semantic_diff(old: &Policy, new: &Policy) -> Vec<String> {
    let mut out = Vec::new();
    // guards
    macro_rules! gdiff {
        ($field:ident) => {
            if old.guards.$field != new.guards.$field {
                out.push(format!(
                    "guards.{}: {:?} -> {:?}",
                    stringify!($field),
                    old.guards.$field,
                    new.guards.$field
                ));
            }
        };
    }
    gdiff!(main_protection);
    gdiff!(branch_origin);
    gdiff!(branch_naming);
    gdiff!(history_protection);
    gdiff!(delete_guard);
    gdiff!(protected_paths);
    gdiff!(policy_guard);
    gdiff!(identity);
    gdiff!(merge_guard);
    gdiff!(push_guard);
    gdiff!(agent_guard);
    if old.gates.len() != new.gates.len() {
        out.push(format!(
            "gates count: {} -> {}",
            old.gates.len(),
            new.gates.len()
        ));
    }
    if old.lock.keys != new.lock.keys {
        out.push(format!(
            "lock.keys: {:?} -> {:?}",
            old.lock.keys, new.lock.keys
        ));
    }
    if old.waivers.len() != new.waivers.len() {
        out.push(format!(
            "waivers count: {} -> {}",
            old.waivers.len(),
            new.waivers.len()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::Policy;

    #[test]
    fn mode_rank() {
        assert!(mode_tightened(Mode::Warn, Mode::Block));
        assert!(!mode_tightened(Mode::Block, Mode::Warn));
    }

    #[test]
    fn tighten_only_default() {
        let base = Policy::default();
        let local = Policy::default();
        assert!(local_tighten_only(&base, &local));
    }

    #[test]
    fn weakening_detected() {
        let base = Policy::default();
        let mut local = Policy::default();
        local.guards.delete_guard = Mode::Off;
        assert!(!local_tighten_only(&base, &local));
    }
}
