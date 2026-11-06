// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Weakening detection (GD-07).
//!
//! The engine compares two *effective* policies and classifies every
//! difference against the strictness table in
//! `plan/03-POLICY-AND-DATA.md` section 4. A change is tightening only when
//! the table says it moves in the strict direction; every other change,
//! including any change the table does not know, is weakening (fail closed).

use crate::config::validate::{compare_versions, duration_millis};
use repowill_model::Policy;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// One difference between two effective policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Dotted path. Entries of id-keyed arrays use the entry id, for example
    /// `gates.tests.mode`; plain arrays use the array path.
    pub path: String,
    /// Human-readable description of the difference.
    pub detail: String,
}

/// Every weakening difference between `old` and `new`.
///
/// An empty result means the change is tightening (or a no-op). Anything the
/// strictness table does not classify is reported as weakening.
pub fn weakening_changes(old: &Policy, new: &Policy) -> Vec<Change> {
    semantic_changes(old, new)
        .into_iter()
        .filter(|change| change.weakening)
        .map(|change| Change {
            path: change.path,
            detail: change.detail,
        })
        .collect()
}

/// True when `new` only tightens `base`.
pub fn local_tighten_only(base: &Policy, new: &Policy) -> bool {
    weakening_changes(base, new).is_empty()
}

/// A human-readable list of every difference between two policies.
///
/// This includes tightening changes and is intended for review output; use
/// [`weakening_changes`] for the security decision.
pub fn semantic_diff(old: &Policy, new: &Policy) -> Vec<String> {
    semantic_changes(old, new)
        .into_iter()
        .map(|change| {
            let direction = if change.weakening {
                "weakening"
            } else {
                "tightening"
            };
            format!("{} ({direction}): {}", change.path, change.detail)
        })
        .collect()
}

struct ClassifiedChange {
    path: String,
    detail: String,
    weakening: bool,
}

fn semantic_changes(old: &Policy, new: &Policy) -> Vec<ClassifiedChange> {
    let (Ok(old), Ok(new)) = (serde_json::to_value(old), serde_json::to_value(new)) else {
        return vec![ClassifiedChange {
            path: "<policy>".to_string(),
            detail: "policy could not be serialized; failing closed".to_string(),
            weakening: true,
        }];
    };
    let mut diffs = Vec::new();
    let mut path = Vec::new();
    collect_diffs(&mut path, &old, &new, &mut diffs);
    diffs
        .into_iter()
        .map(|diff| {
            let path = diff.path().join(".");
            ClassifiedChange {
                weakening: classify(&diff),
                detail: diff.detail(),
                path,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Structural diff
// ---------------------------------------------------------------------------

enum Diff {
    Added {
        path: Vec<String>,
        value: Value,
    },
    Removed {
        path: Vec<String>,
        value: Value,
    },
    Modified {
        path: Vec<String>,
        old: Value,
        new: Value,
    },
}

impl Diff {
    fn path(&self) -> &[String] {
        match self {
            Self::Added { path, .. } | Self::Removed { path, .. } | Self::Modified { path, .. } => {
                path
            }
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Added { value, .. } => format!("added {}", compact(value)),
            Self::Removed { value, .. } => format!("removed {}", compact(value)),
            Self::Modified { old, new, .. } => {
                format!("{} -> {}", compact(old), compact(new))
            }
        }
    }
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unprintable>".to_string())
}

fn collect_diffs(path: &mut Vec<String>, old: &Value, new: &Value, out: &mut Vec<Diff>) {
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => {
            for key in old.keys() {
                if !new.contains_key(key) {
                    path.push(key.clone());
                    out.push(Diff::Removed {
                        path: path.clone(),
                        value: old[key].clone(),
                    });
                    path.pop();
                }
            }
            for (key, new_value) in new {
                path.push(key.clone());
                match old.get(key) {
                    Some(old_value) => collect_diffs(path, old_value, new_value, out),
                    None => out.push(Diff::Added {
                        path: path.clone(),
                        value: new_value.clone(),
                    }),
                }
                path.pop();
            }
        }
        (Value::Array(old), Value::Array(new))
            if is_id_keyed(old, new) && parent_is_id_array(path) =>
        {
            collect_id_array_diffs(path, old, new, out);
        }
        _ if old == new => {}
        _ => out.push(Diff::Modified {
            path: path.clone(),
            old: old.clone(),
            new: new.clone(),
        }),
    }
}

fn parent_is_id_array(path: &[String]) -> bool {
    crate::config::value::is_id_array_path(path)
}

fn is_id_keyed(old: &[Value], new: &[Value]) -> bool {
    !old.is_empty() || !new.is_empty()
}

fn collect_id_array_diffs(
    path: &mut Vec<String>,
    old: &[Value],
    new: &[Value],
    out: &mut Vec<Diff>,
) {
    let old_by_id = by_id(old);
    let new_by_id = by_id(new);
    let Some(old_by_id) = old_by_id else {
        out.push(Diff::Modified {
            path: path.clone(),
            old: Value::Array(old.to_vec()),
            new: Value::Array(new.to_vec()),
        });
        return;
    };
    let Some(new_by_id) = new_by_id else {
        out.push(Diff::Modified {
            path: path.clone(),
            old: Value::Array(old.to_vec()),
            new: Value::Array(new.to_vec()),
        });
        return;
    };
    for (id, old_value) in &old_by_id {
        if !new_by_id.contains_key(id) {
            path.push((*id).to_string());
            out.push(Diff::Removed {
                path: path.clone(),
                value: (*old_value).clone(),
            });
            path.pop();
        }
    }
    for (id, new_value) in &new_by_id {
        path.push((*id).to_string());
        match old_by_id.get(id) {
            Some(old_value) => collect_diffs(path, old_value, new_value, out),
            None => out.push(Diff::Added {
                path: path.clone(),
                value: (*new_value).clone(),
            }),
        }
        path.pop();
    }
}

/// Index an id-keyed array. Returns `None` when any entry is not an object
/// with a non-empty string `id`, in which case the array is compared
/// wholesale (fail closed).
fn by_id(items: &[Value]) -> Option<BTreeMap<&str, &Value>> {
    let mut map = BTreeMap::new();
    for item in items {
        let id = item.get("id").and_then(Value::as_str)?;
        if id.is_empty() {
            return None;
        }
        map.insert(id, item);
    }
    Some(map)
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

fn classify(diff: &Diff) -> bool {
    match diff {
        Diff::Added { path, value } => added_weakens(path, value),
        Diff::Removed { path, value } => removed_weakens(path, value),
        Diff::Modified { path, old, new } => modified_weakens(path, old, new),
    }
}

fn added_weakens(path: &[String], value: &Value) -> bool {
    let Some((key, parent)) = path.split_last() else {
        return true;
    };
    if parent_is_id_array(parent) {
        return id_entry_added_weakens(&parent.join("."));
    }
    if parent.last().is_some_and(|segment| segment == "with") {
        return !matches!(key.as_str(), "patterns" | "ids" | "include");
    }
    let _ = value;
    // Adding a key the table does not know is weakening (fail closed).
    true
}

fn removed_weakens(path: &[String], value: &Value) -> bool {
    let Some((key, parent)) = path.split_last() else {
        return true;
    };
    if parent_is_id_array(parent) {
        return id_entry_removed_weakens(&parent.join("."));
    }
    if parent.last().is_some_and(|segment| segment == "with") {
        return !matches!(
            key.as_str(),
            "exclude" | "ignore" | "allow" | "success_codes" | "warn_codes"
        );
    }
    let _ = value;
    true
}

fn id_entry_added_weakens(array: &str) -> bool {
    matches!(
        array,
        "dependencies.exceptions" | "waivers" | "identity.allowed"
    )
}

fn id_entry_removed_weakens(array: &str) -> bool {
    !matches!(
        array,
        "dependencies.exceptions" | "waivers" | "identity.allowed"
    )
}

fn modified_weakens(path: &[String], old: &Value, new: &Value) -> bool {
    if old == new {
        return false;
    }
    match (old, new) {
        (Value::String(old), Value::String(new)) => string_weakens(path, old, new),
        (Value::Bool(old), Value::Bool(new)) => bool_weakens(path, *old, *new),
        (Value::Number(old), Value::Number(new)) => number_weakens(path, old, new),
        (Value::Array(old), Value::Array(new)) => array_weakens(path, old, new),
        _ => true,
    }
}

fn last(path: &[String]) -> &str {
    path.last().map(String::as_str).unwrap_or("")
}

fn string_weakens(path: &[String], old: &str, new: &str) -> bool {
    // Generic mode fields: block > warn > off.
    if let (Some(old), Some(new)) = (mode_rank(old), mode_rank(new)) {
        return new < old;
    }
    // Vulnerability severity thresholds: lower block_at/warn_at is stricter.
    if let (Some(old), Some(new)) = (severity_rank(old), severity_rank(new))
        && matches!(last(path), "block_at" | "warn_at")
    {
        return new > old;
    }
    match last(path) {
        "unknown_severity" => rank_decreased(old, new, &["ignore", "warn", "block"]),
        "files" | "branches" | "tags" if path.iter().any(|segment| segment == "delete") => {
            rank_decreased(old, new, &["off", "warn", "approval"])
        }
        "mode" if path.iter().any(|segment| segment == "protected_paths") => {
            rank_decreased(old, new, &["warn", "approval", "immutable"])
        }
        "outside_workspace" => rank_decreased(old, new, &["ask", "deny"]),
        "unknown_command" if path.iter().any(|segment| segment == "guard") => {
            rank_decreased(old, new, &["allow", "ask"])
        }
        "mode" if path.iter().any(|segment| segment == "parallel") => {
            rank_decreased(old, new, &["worktrees", "forbid"])
        }
        "strategy" if path.iter().any(|segment| segment == "merge") => {
            rank_decreased(old, new, &["squash", "ff-only", "rebase-merge", "no-ff"])
        }
        "up_to_date" => rank_decreased(old, new, &["off", "contains-main", "trial-merge"]),
        "trigger" => rank_decreased(old, new, &["manual", "auto"]),
        "approval_source" => rank_decreased(old, new, &["github-review", "either", "local"]),
        "allow_force" => rank_decreased(old, new, &["own-unmerged-with-lease", "never"]),
        "protected_direct" => rank_decreased(old, new, &["after-local-merge", "never"]),
        "remote_deletions" => rank_decreased(old, new, &["approval", "never"]),
        "convention" => rank_decreased(old, new, &["free", "custom", "conventional"]),
        "atomic" if path.iter().any(|segment| segment == "commits") => {
            rank_decreased(old, new, &["off", "standard", "strict"])
        }
        "check_on" => rank_decreased(old, new, &["manual", "daily", "every-hook"]),
        "applies_to" => rank_decreased(old, new, &["direct", "all"]),
        "tolerance" => rank_decreased(old, new, &["major", "minor", "patch", "exact"]),
        "min_repowill_version" => compare_versions(new, old) != Some(Ordering::Greater),
        "weakening" if path.iter().any(|segment| segment == "policy") => {
            rank_decreased(old, new, &["warn", "approval"])
        }
        "mode" if path.iter().any(|segment| segment == "network") => {
            rank_decreased(old, new, &["online", "cache-only", "offline"])
        }
        "timeout" | "default_timeout" => {
            match (duration_millis(old), duration_millis(new)) {
                // A longer timeout lets a check run longer; it is stricter to
                // fail sooner, so only a reduced timeout is tightening.
                (Some(old), Some(new)) => new > old,
                // Unparseable durations are schema errors; fail closed.
                _ => true,
            }
        }
        "expires"
            if path.iter().any(|segment| segment == "waivers")
                || path.iter().any(|segment| segment == "exceptions") =>
        {
            // Dates are YYYY-MM-DD and sort lexicographically. An extended
            // expiry keeps an exception alive longer: weakening.
            new > old
        }
        _ => true,
    }
}

fn rank_decreased(old: &str, new: &str, order: &[&str]) -> bool {
    let Some(old) = order.iter().position(|entry| *entry == old) else {
        return true;
    };
    let Some(new) = order.iter().position(|entry| *entry == new) else {
        return true;
    };
    new < old
}

fn mode_rank(value: &str) -> Option<u8> {
    match value {
        "off" => Some(0),
        "warn" => Some(1),
        "block" => Some(2),
        _ => None,
    }
}

fn severity_rank(value: &str) -> Option<u8> {
    match value {
        "none" => Some(0),
        "low" => Some(1),
        "medium" => Some(2),
        "high" => Some(3),
        "critical" => Some(4),
        _ => None,
    }
}

/// Booleans that invert the usual "true is stricter" polarity.
const INVERTED_BOOLEANS: &[&str] = &[
    "allow_stacked",
    "allow_prerelease",
    "allow_git",
    "allow_url",
    "allow_path",
    "allow_local_weakening",
    "honor_inline_allow",
    "lfs_ok",
    "only_changed_files",
];

fn bool_weakens(path: &[String], old: bool, new: bool) -> bool {
    let name = last(path);
    // `remove = true` on an effective policy entry means the entry is gone,
    // which the structural diff reports separately. When both forms exist,
    // treat removal of a rule/allowlist entry in the strict direction.
    if name == "remove" {
        let allowlist = path
            .iter()
            .any(|segment| segment == "waivers" || segment == "exceptions");
        return if allowlist { old && !new } else { !old && new };
    }
    if INVERTED_BOOLEANS.contains(&name) {
        !old && new
    } else {
        old && !new
    }
}

/// Numeric fields for which raising the value weakens the policy.
const RAISED_WEAKENS: &[&str] = &[
    "staleness_days",
    "subject_max",
    "body_max_line",
    "atomic_max_files",
    "atomic_max_lines",
    "stale_branch_days",
    "max_commits_ahead",
    "max_feedback_lines",
    "max_file_size_mb",
    "max_kb",
    "max_path",
    "max_warnings",
    "retries",
    "cache_ttl_hours",
    "unmaintained_days",
    "outdated_grace_days",
    "approval_expiry_hours",
    "interval_minutes",
    "ratchet_tolerance",
];

/// Numeric fields for which lowering the value weakens the policy.
const LOWERED_WEAKENS: &[&str] = &[
    "min_bytes",
    "min_lines",
    "min_branches",
    "min_release_age_days",
    "retention_days",
    "max_count",
    "entropy_min",
];

fn number_weakens(path: &[String], old: &serde_json::Number, new: &serde_json::Number) -> bool {
    let (Some(old), Some(new)) = (old.as_f64(), new.as_f64()) else {
        return true;
    };
    let name = last(path);
    if RAISED_WEAKENS.contains(&name) {
        new > old
    } else if LOWERED_WEAKENS.contains(&name) {
        new < old
    } else {
        // Unknown numeric direction: fail closed.
        true
    }
}

fn array_weakens(path: &[String], old: &[Value], new: &[Value]) -> bool {
    let name = last(path);
    match name {
        // Adding an allowed license, registry, ecosystem, allowed base,
        // branch type, commit type, allowed scope, allowed reference, or
        // waived path widens an allowlist: weakening.
        "allowed" | "content_licenses" | "allowed_registries" | "ecosystems" | "allowed_bases"
        | "types" | "scopes" | "only_on" | "allow_patterns" | "stacked_patterns" | "allow_read"
        | "enabled" => !new_subset_of_old(old, new),
        "exceptions" => !new_subset_of_old(old, new),
        "allow" => !new_subset_of_old(old, new),
        // Optional waiver paths narrow a waiver. An empty list means the
        // waiver applies everywhere, so adding paths is tightening.
        "paths" if path.iter().any(|segment| segment == "waivers") => {
            waiver_scope_weakens(old, new)
        }
        "paths" => !new_subset_of_old(old, new),
        // Removing a denied license or a required doc/trailer/body rule
        // narrows the policy: weakening.
        "denied" | "required" | "require_body_for" | "required_trailers" | "requested_trailers"
        | "deny_read" | "gate_tags" | "outputs" | "when_changed" | "require_changed"
        | "require_for" | "protected_branches" => !old_subset_of_new(old, new),
        "include" if path.iter().any(|segment| segment == "paths") => !old_subset_of_new(old, new),
        // An empty branch/profile/OS list means "all", so adding entries
        // narrows the gate's application (weakening) while removing entries
        // widens it (tightening).
        "branches" | "profiles" | "os" if path.iter().any(|segment| segment == "gates") => {
            universal_narrowed(old, new)
        }
        "stages" | "tags" | "depends_on" if path.iter().any(|segment| segment == "gates") => {
            !old_subset_of_new(old, new)
        }
        "keys" if path.iter().any(|segment| segment == "lock") => !old_subset_of_new(old, new),
        "stacks" if path.iter().any(|segment| segment == "project") => !new_subset_of_old(old, new),
        "success_codes" | "warn_codes" | "ignore" => !new_value_subset_of_old(old, new),
        "patterns" | "ids" | "include" => !old_subset_of_new(old, new),
        "exclude" => !new_subset_of_old(old, new),
        "args" | "source" | "tests" | "per_path" => true,
        _ => true,
    }
}

fn values(value: &[Value]) -> Option<Vec<&str>> {
    value.iter().map(Value::as_str).collect()
}

/// Every `old` entry is covered by some `new` entry.
fn old_subset_of_new(old: &[Value], new: &[Value]) -> bool {
    let (Some(old), Some(new)) = (values(old), values(new)) else {
        return false;
    };
    covered_by(&new, &old)
}

/// Every `new` entry is covered by some `old` entry.
fn new_subset_of_old(old: &[Value], new: &[Value]) -> bool {
    let (Some(old), Some(new)) = (values(old), values(new)) else {
        return false;
    };
    covered_by(&old, &new)
}

fn covered_by(candidates: &[&str], targets: &[&str]) -> bool {
    targets.iter().all(|target| {
        candidates
            .iter()
            .any(|candidate| glob_covers(candidate, target))
    })
}

/// Conservative glob containment: true only when every path matched by
/// `covered` is also matched by `covering` for the forms this engine emits.
fn glob_covers(covering: &str, covered: &str) -> bool {
    if covering == covered || covering == "**" || covering == "**/*" {
        return true;
    }
    if let Some(prefix) = covering.strip_suffix("/**") {
        let covered = covered.trim_end_matches("/**").trim_end_matches("/*");
        return covered == prefix || covered.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = covering.strip_suffix("/*")
        && let Some(rest) = covered.strip_prefix(&format!("{prefix}/"))
    {
        return !rest.contains('/');
    }
    false
}

fn new_value_subset_of_old(old: &[Value], new: &[Value]) -> bool {
    new.iter().all(|item| old.contains(item))
}

/// True when a list whose empty value means "everything" became narrower.
fn universal_narrowed(old: &[Value], new: &[Value]) -> bool {
    let (Some(old), Some(new)) = (values(old), values(new)) else {
        return true;
    };
    if old.is_empty() {
        return !new.is_empty();
    }
    if new.is_empty() {
        return false;
    }
    !covered_by(&new, &old)
}

/// True when a scope list that defaults to "everything" widened.
fn waiver_scope_weakens(old: &[Value], new: &[Value]) -> bool {
    let (Some(old), Some(new)) = (values(old), values(new)) else {
        return true;
    };
    if new.is_empty() {
        return !old.is_empty();
    }
    if old.is_empty() {
        return false;
    }
    !covered_by(&old, &new)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::{Gate, GatePaths, Mode};
    use serde_json::json;

    fn gate(id: &str, stages: &[&str]) -> Gate {
        Gate {
            id: id.into(),
            name: format!("Gate {id}"),
            description: String::new(),
            uses: "builtin:dco".into(),
            with: Value::Null,
            stages: stages.iter().map(|stage| stage.to_string()).collect(),
            mode: Mode::Block,
            stage_modes: BTreeMap::new(),
            trigger: "auto".into(),
            on_error: "block".into(),
            timeout: "10m".into(),
            paths: GatePaths {
                include: vec!["**".into()],
                exclude: vec![],
            },
            run_always: false,
            depends_on: vec![],
            profiles: vec![],
            branches: vec![],
            only_in: "any".into(),
            os: vec![],
            tags: vec![],
            locked: false,
            remove: false,
        }
    }

    fn policy_with_gate() -> Policy {
        let mut policy = Policy::default();
        policy.gates.push(gate("tests", &["pre-commit"]));
        policy
    }

    fn assert_tightening(old: &Policy, new: &Policy) {
        let changes = weakening_changes(old, new);
        assert!(
            changes.is_empty(),
            "expected tightening, got weakening: {changes:?}"
        );
    }

    fn assert_weakening(old: &Policy, new: &Policy) {
        assert!(
            !weakening_changes(old, new).is_empty(),
            "expected weakening, got tightening; diff: {:?}",
            semantic_diff(old, new)
        );
    }

    #[test]
    fn identical_policies_have_no_changes() {
        let policy = policy_with_gate();
        assert!(semantic_diff(&policy, &policy).is_empty());
        assert!(local_tighten_only(&policy, &policy));
    }

    #[test]
    fn unknown_change_is_weakening() {
        let old = Policy::default();
        let mut new = Policy::default();
        new.project.description = "local note".into();
        assert_weakening(&old, &new);
    }

    #[test]
    fn mode_matrix() {
        let modes = [Mode::Block, Mode::Warn, Mode::Off];
        let rank = |mode| match mode {
            Mode::Block => 2,
            Mode::Warn => 1,
            Mode::Off => 0,
        };
        for old_mode in modes {
            for new_mode in modes {
                let mut old = Policy::default();
                old.guards.delete_guard = old_mode;
                let mut new = Policy::default();
                new.guards.delete_guard = new_mode;
                if rank(new_mode) < rank(old_mode) {
                    assert_weakening(&old, &new);
                } else {
                    assert_tightening(&old, &new);
                }
            }
        }
    }

    #[test]
    fn mode_order_is_transitive() {
        for a in [Mode::Block, Mode::Warn, Mode::Off] {
            for b in [Mode::Block, Mode::Warn, Mode::Off] {
                for c in [Mode::Block, Mode::Warn, Mode::Off] {
                    if !mode_tightened(a, b) || !mode_tightened(b, c) {
                        continue;
                    }
                    assert!(mode_tightened(a, c));
                }
            }
        }
    }

    fn mode_tightened(old: Mode, new: Mode) -> bool {
        let rank = |mode| match mode {
            Mode::Block => 2,
            Mode::Warn => 1,
            Mode::Off => 0,
        };
        rank(new) >= rank(old)
    }

    #[test]
    fn gates_added_and_removed() {
        let empty = Policy::default();
        let with_gate = policy_with_gate();
        assert_tightening(&empty, &with_gate);
        assert_weakening(&with_gate, &empty);
    }

    #[test]
    fn stage_removal_is_weakening() {
        let mut old = policy_with_gate();
        old.gates[0].stages = vec!["pre-commit".into(), "pre-push".into()];
        assert_weakening(&old, &policy_with_gate());
        assert_tightening(&policy_with_gate(), &old);
    }

    #[test]
    fn path_include_and_exclude() {
        let old = policy_with_gate();
        let mut wider = old.clone();
        wider.gates[0].paths.include = vec!["**".into()];
        assert_tightening(&old, &wider);

        let mut narrower = old.clone();
        narrower.gates[0].paths.include = vec!["src/**".into()];
        assert_weakening(&old, &narrower);

        let mut excluded = old.clone();
        excluded.gates[0].paths.exclude = vec!["vendor/**".into()];
        assert_weakening(&old, &excluded);
    }

    #[test]
    fn trigger_and_error_mode() {
        let old = policy_with_gate();
        let mut manual = old.clone();
        manual.gates[0].trigger = "manual".into();
        assert_weakening(&old, &manual);
        assert_tightening(&manual, &old);

        let mut warn = old.clone();
        warn.gates[0].on_error = "warn".into();
        assert_weakening(&old, &warn);
        assert_tightening(&warn, &old);
    }

    #[test]
    fn numeric_limits() {
        let old = policy_with_gate();
        let mut looser = old.clone();
        looser.workflow.commits.subject_max = 100;
        assert_weakening(&old, &looser);

        let mut stricter = old.clone();
        stricter.workflow.commits.subject_max = 60;
        assert_tightening(&old, &stricter);

        let mut shorter = old.clone();
        shorter.dependencies.versions.min_release_age_days = 1;
        assert_weakening(&old, &shorter);

        let mut longer_approval = old.clone();
        longer_approval.workflow.merge.approval_expiry_hours = 72;
        assert_weakening(&old, &longer_approval);

        let mut shorter_retention = old.clone();
        shorter_retention.protection.snapshots.retention_days = 1;
        assert_weakening(&old, &shorter_retention);
    }

    #[test]
    fn severity_thresholds() {
        let old = policy_with_gate();
        let mut looser = old.clone();
        looser.dependencies.vulnerabilities.block_at = "critical".into();
        assert_weakening(&old, &looser);

        let mut stricter = old.clone();
        stricter.dependencies.vulnerabilities.block_at = "medium".into();
        assert_tightening(&old, &stricter);
    }

    #[test]
    fn license_lists() {
        let old = policy_with_gate();
        let mut added = old.clone();
        added.dependencies.licenses.allowed.push("WTFPL".into());
        assert_weakening(&old, &added);

        let mut denied = old.clone();
        denied.dependencies.licenses.denied = vec!["GPL-3.0-only".into()];
        let mut removed_denied = denied.clone();
        removed_denied.dependencies.licenses.denied = vec![];
        assert_weakening(&denied, &removed_denied);
    }

    #[test]
    fn dependency_version_rules() {
        let old = policy_with_gate();
        let mut all = old.clone();
        all.dependencies.versions.applies_to = "all".into();
        let mut direct = all.clone();
        direct.dependencies.versions.applies_to = "direct".into();
        assert_weakening(&all, &direct);
        assert_tightening(&direct, &all);

        let mut looser = old.clone();
        looser.dependencies.versions.tolerance = "major".into();
        assert_weakening(&old, &looser);

        let mut prerelease = old.clone();
        prerelease.dependencies.versions.allow_prerelease = true;
        assert_weakening(&old, &prerelease);

        let mut registry = old.clone();
        registry
            .dependencies
            .sources
            .allowed_registries
            .push("evil.example".into());
        assert_weakening(&old, &registry);
    }

    #[test]
    fn workflow_rules() {
        let old = policy_with_gate();

        let mut unprotected = old.clone();
        unprotected.workflow.protected_branches.clear();
        assert_weakening(&old, &unprotected);

        let mut stacked = old.clone();
        stacked.workflow.allow_stacked = true;
        assert_weakening(&old, &stacked);

        let mut no_approval = old.clone();
        no_approval.workflow.merge.require_approval = false;
        assert_weakening(&old, &no_approval);

        let mut squash = old.clone();
        squash.workflow.merge.strategy = repowill_model::MergeStrategy::Squash;
        assert_weakening(&old, &squash);

        let mut force = old.clone();
        force.workflow.push.allow_force = repowill_model::AllowForce::OwnUnmergedWithLease;
        assert_weakening(&old, &force);

        let mut stale = old.clone();
        stale.workflow.merge.up_to_date = repowill_model::UpToDate::Off;
        assert_weakening(&old, &stale);

        let mut remote_approval = old.clone();
        remote_approval.workflow.merge.approval_source =
            repowill_model::ApprovalSource::GithubReview;
        assert_weakening(&old, &remote_approval);
    }

    #[test]
    fn agent_guard_rules() {
        let old = policy_with_gate();
        let mut delete = old.clone();
        delete.agents.guard.block_delete = false;
        assert_weakening(&old, &delete);

        let mut deny = old.clone();
        deny.agents.guard.deny_read.clear();
        assert_weakening(&old, &deny);

        let mut outside = old.clone();
        outside.agents.guard.outside_workspace = "ask".into();
        assert_weakening(&old, &outside);

        let mut old = policy_with_gate();
        let mut unknown = old.clone();
        old.agents.guard.unknown_command = "ask".into();
        unknown.agents.guard.unknown_command = "allow".into();
        assert_weakening(&old, &unknown);
    }

    #[test]
    fn snapshot_and_integrity_rules() {
        let old = policy_with_gate();
        let mut disabled = old.clone();
        disabled.protection.snapshots.enabled = false;
        assert_weakening(&old, &disabled);

        let mut warn = old.clone();
        warn.protection.integrity.on_violation = repowill_model::OnViolation::Warn;
        assert_weakening(&old, &warn);
    }

    #[test]
    fn allowlists_and_waivers() {
        let old = policy_with_gate();
        let mut waiver = old.clone();
        waiver.waivers.push(repowill_model::Waiver {
            id: "w".into(),
            target: "gates.tests".into(),
            reason: "temporary".into(),
            paths: vec![],
            expires: "2026-12-31".into(),
            request: String::new(),
            remove: false,
        });
        assert_weakening(&old, &waiver);
        assert_tightening(&waiver, &old);

        let mut extended = waiver.clone();
        extended.waivers[0].expires = "2027-12-31".into();
        assert_weakening(&waiver, &extended);

        let mut allowed = old.clone();
        allowed
            .identity
            .allowed
            .push(repowill_model::IdentityEntry {
                id: "eve".into(),
                name: "Eve".into(),
                email: "eve@example.com".into(),
                remove: false,
            });
        assert_weakening(&old, &allowed);
    }

    #[test]
    fn lock_keys_can_only_be_added() {
        let old = policy_with_gate();
        let mut locked = old.clone();
        locked.lock.keys = vec!["guards.main_protection".into()];
        assert_tightening(&old, &locked);
        assert_weakening(&locked, &old);
    }

    #[test]
    fn gate_with_parameters() {
        let old = policy_with_gate();
        let mut raised = old.clone();
        raised.gates[0].uses = "builtin:large-files".into();
        raised.gates[0].with = json!({ "max_kb": 1000 });
        let mut higher = raised.clone();
        higher.gates[0].with = json!({ "max_kb": 4000 });
        assert_weakening(&raised, &higher);

        let mut excluded = old.clone();
        excluded.gates[0].paths.exclude = vec!["dist/**".into()];
        assert_weakening(&old, &excluded);

        let mut full = old.clone();
        full.gates[0].uses = "builtin:dep-licenses".into();
        full.gates[0].with = json!({ "full": true });
        let mut incremental = full.clone();
        incremental.gates[0].with = json!({ "full": false });
        assert_weakening(&full, &incremental);
    }

    #[test]
    fn unknown_change_fails_closed_even_when_it_looks_neutral() {
        let old = policy_with_gate();
        let mut changed = old.clone();
        changed.engine.concurrency = 8;
        assert_weakening(&old, &changed);
    }

    #[test]
    fn glob_containment() {
        assert!(glob_covers("**", "src/**"));
        assert!(glob_covers("**/*", "src/a.rs"));
        assert!(glob_covers("src/**", "src/a/b.rs"));
        assert!(glob_covers("src/**", "src/a/**"));
        assert!(glob_covers("src/*", "src/a.rs"));
        assert!(!glob_covers("src/*", "src/a/b.rs"));
        assert!(!glob_covers("src/a/**", "src/**"));
        assert!(!glob_covers("tests/**", "src/**"));
    }

    #[test]
    fn semantic_diff_labels_changes() {
        let old = policy_with_gate();
        let mut new = old.clone();
        new.gates[0].mode = Mode::Warn;
        let diff = semantic_diff(&old, &new);
        assert_eq!(diff.len(), 1);
        assert!(diff[0].contains("gates.tests.mode"));
        assert!(diff[0].contains("weakening"));
    }
}
