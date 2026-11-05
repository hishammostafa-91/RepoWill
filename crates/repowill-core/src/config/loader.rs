// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use repowill_model::Policy;

pub fn load_policy_str(src: &str) -> Result<Policy, String> {
    toml::from_str(src).map_err(|e| e.to_string())
}

/// Merge at the TOML value level: recursively merge tables, replace arrays, honour `remove = true`
/// in id-bearing array entries, and union `[lock].keys`. Locked keys are not overwritten.
pub fn merge_layer(base: &mut Policy, layer: &Policy) {
    // Field-by-field merge (value-level merge_values helper retained for future unification).
    if !layer.project.name.is_empty() {
        base.project.name = layer.project.name.clone();
    }
    if !layer.project.description.is_empty() {
        base.project.description = layer.project.description.clone();
    }
    if !layer.project.license.is_empty() {
        base.project.license = layer.project.license.clone();
    }
    if !layer.project.stacks.is_empty() {
        base.project.stacks = layer.project.stacks.clone();
    }
    if !layer.project.preset.is_empty() {
        base.project.preset = layer.project.preset.clone();
    }
    if !layer.project.profile.is_empty() {
        base.project.profile = layer.project.profile.clone();
    }

    // identity
    if !layer.identity.name.is_empty() {
        base.identity.name = layer.identity.name.clone();
    }
    if !layer.identity.email.is_empty() {
        base.identity.email = layer.identity.email.clone();
    }
    base.identity.enforce = layer.identity.enforce;
    base.identity.check_committer = layer.identity.check_committer;
    base.identity.signing = layer.identity.signing;
    for e in &layer.identity.allowed {
        if e.remove {
            base.identity.allowed.retain(|x| x.id != e.id);
        } else if let Some(x) = base.identity.allowed.iter_mut().find(|x| x.id == e.id) {
            *x = e.clone();
        } else {
            base.identity.allowed.push(e.clone());
        }
    }

    // workflow scalars
    if layer.workflow.model != Default::default() {
        base.workflow.model = layer.workflow.model;
    }
    if !layer.workflow.main_branch.is_empty() {
        base.workflow.main_branch = layer.workflow.main_branch.clone();
    }
    if !layer.workflow.protected_branches.is_empty() {
        base.workflow.protected_branches = layer.workflow.protected_branches.clone();
    }
    if !layer.workflow.integration_branch.is_empty() {
        base.workflow.integration_branch = layer.workflow.integration_branch.clone();
    }
    if !layer.workflow.allowed_bases.is_empty() {
        base.workflow.allowed_bases = layer.workflow.allowed_bases.clone();
    }
    base.workflow.branch_from_tip = layer.workflow.branch_from_tip || base.workflow.branch_from_tip;
    base.workflow.allow_stacked = layer.workflow.allow_stacked || base.workflow.allow_stacked;
    if !layer.workflow.stacked_patterns.is_empty() {
        base.workflow.stacked_patterns = layer.workflow.stacked_patterns.clone();
    }
    if layer.workflow.stale_branch_days != 0 {
        base.workflow.stale_branch_days = layer.workflow.stale_branch_days;
    }
    if layer.workflow.max_commits_ahead != 0 {
        base.workflow.max_commits_ahead = layer.workflow.max_commits_ahead;
    }

    // guards
    base.guards = layer.guards.clone();

    // protection: protected_paths merge by id (with remove)
    for pp in &layer.protection.protected_paths {
        if pp.remove {
            base.protection.protected_paths.retain(|x| x.id != pp.id);
        } else if let Some(e) = base
            .protection
            .protected_paths
            .iter_mut()
            .find(|x| x.id == pp.id)
        {
            *e = pp.clone();
        } else {
            base.protection.protected_paths.push(pp.clone());
        }
    }
    base.protection.delete = layer.protection.delete.clone();
    base.protection.policy = layer.protection.policy.clone();
    base.protection.snapshots = layer.protection.snapshots.clone();
    base.protection.integrity = layer.protection.integrity.clone();

    // dependencies
    base.dependencies = layer.dependencies.clone();

    // docs
    for c in &layer.docs.coupling {
        if c.remove {
            base.docs.coupling.retain(|x| x.id != c.id);
        } else if let Some(e) = base.docs.coupling.iter_mut().find(|x| x.id == c.id) {
            *e = c.clone();
        } else {
            base.docs.coupling.push(c.clone());
        }
    }
    for g in &layer.docs.generated {
        if g.remove {
            base.docs.generated.retain(|x| x.id != g.id);
        } else if let Some(e) = base.docs.generated.iter_mut().find(|x| x.id == g.id) {
            *e = g.clone();
        } else {
            base.docs.generated.push(g.clone());
        }
    }
    if !layer.docs.required.is_empty() {
        base.docs.required = layer.docs.required.clone();
    }
    base.docs.staleness_days = layer.docs.staleness_days;
    base.docs.changelog = layer.docs.changelog.clone();

    // agents
    if !layer.agents.enabled.is_empty() {
        base.agents.enabled = layer.agents.enabled.clone();
    }
    base.agents.guard = layer.agents.guard.clone();
    base.agents.stop_check = layer.agents.stop_check.clone();
    base.agents.parallel = layer.agents.parallel.clone();

    // remote
    if !layer.remote.provider.is_empty() {
        base.remote.provider = layer.remote.provider.clone();
    }
    if !layer.remote.owner.is_empty() {
        base.remote.owner = layer.remote.owner.clone();
    }
    if !layer.remote.repo.is_empty() {
        base.remote.repo = layer.remote.repo.clone();
    }
    base.remote.remote_name = layer.remote.remote_name.clone();
    base.remote.visibility = layer.remote.visibility.clone();
    base.remote.github = layer.remote.github.clone();

    // engine
    base.engine = layer.engine.clone();

    // gates
    for g in &layer.gates {
        if g.remove {
            base.gates.retain(|x| x.id != g.id);
        } else if let Some(existing) = base.gates.iter_mut().find(|x| x.id == g.id) {
            *existing = g.clone();
        } else {
            base.gates.push(g.clone());
        }
    }

    // waivers
    for w in &layer.waivers {
        if w.remove {
            base.waivers.retain(|x| x.id != w.id);
        } else if let Some(existing) = base.waivers.iter_mut().find(|x| x.id == w.id) {
            *existing = w.clone();
        } else {
            base.waivers.push(w.clone());
        }
    }

    // lock union
    for k in &layer.lock.keys {
        if !base.lock.keys.contains(k) {
            base.lock.keys.push(k.clone());
        }
    }
}

pub fn load_layers(sources: &[&str]) -> Result<Policy, String> {
    let mut policy = Policy::default();
    for src in sources {
        let layer = load_policy_str(src)?;
        merge_layer(&mut policy, &layer);
    }
    Ok(policy)
}

pub fn load_policy_file(path: &std::path::Path) -> Result<Policy, String> {
    let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    load_policy_str(&src)
}

pub fn load_with_extends(path: &std::path::Path) -> Result<Policy, String> {
    let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let base_policy = load_policy_str(&src)?;
    let mut policy = Policy::default();
    for ext in &base_policy.extends {
        let ext_path = path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(ext);
        if ext_path.exists() {
            let ext_policy = load_with_extends(&ext_path)?;
            merge_layer(&mut policy, &ext_policy);
        }
    }
    merge_layer(&mut policy, &base_policy);
    Ok(policy)
}

pub fn load_local(base: &Policy, local_src: &str) -> Result<Policy, String> {
    let local = load_policy_str(local_src)?;
    if !crate::config::strictness::local_tighten_only(base, &local) {
        return Err("local layer weakens policy (RW-E2104)".to_string());
    }
    let mut merged = base.clone();
    merge_layer(&mut merged, &local);
    Ok(merged)
}

fn get_path<'a>(val: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut cur = val;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

/// Merge a layer, refusing to change any key listed in `base.lock.keys`.
pub fn merge_layer_checked(base: &mut Policy, layer: &Policy) -> Result<(), String> {
    // Compute merged trial.
    let mut trial = base.clone();
    merge_layer(&mut trial, layer);
    // Check locked keys unchanged.
    let base_val = toml::Value::try_from(base.clone()).map_err(|e| e.to_string())?;
    let trial_val = toml::Value::try_from(trial.clone()).map_err(|e| e.to_string())?;
    for key in &base.lock.keys {
        let before = get_path(&base_val, key);
        let after = get_path(&trial_val, key);
        if before != after {
            return Err(format!("locked key changed: {}", key));
        }
    }
    *base = trial;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::Policy;

    #[test]
    fn load_minimal() {
        let p = load_policy_str("schema_version = 1\n").unwrap();
        assert_eq!(p.schema_version, 1);
    }

    #[test]
    fn merge_gates_by_id() {
        let mut base = Policy::default();
        let layer_src = r#"
schema_version = 1
[[gates]]
id = "tests"
name = "Tests"
uses = "builtin:tests"
stages = ["pre-commit"]
"#;
        let layer = load_policy_str(layer_src).unwrap();
        merge_layer(&mut base, &layer);
        assert_eq!(base.gates.len(), 1);
        merge_layer(&mut base, &layer);
        assert_eq!(base.gates.len(), 1);
    }

    #[test]
    fn lock_union() {
        let mut base = Policy::default();
        let layer = load_policy_str("schema_version = 1\n[lock]\nkeys = [\"a\"]\n").unwrap();
        merge_layer(&mut base, &layer);
        assert_eq!(base.lock.keys, vec!["a".to_string()]);
        let layer2 = load_policy_str("schema_version = 1\n[lock]\nkeys = [\"b\"]\n").unwrap();
        merge_layer(&mut base, &layer2);
        assert!(base.lock.keys.contains(&"a".to_string()));
        assert!(base.lock.keys.contains(&"b".to_string()));
    }

    #[test]
    fn load_layers_works() {
        let p = load_layers(&[
            "schema_version = 1\n",
            "schema_version = 1\n[project]\nname = \"x\"\n",
        ])
        .unwrap();
        assert_eq!(p.project.name, "x");
    }

    #[test]
    fn remove_gate_via_layer() {
        let mut base = Policy::default();
        let layer = load_policy_str(
            r#"
schema_version = 1
[[gates]]
id = "t"
name = "T"
uses = "builtin:t"
stages = ["pre-commit"]
"#,
        )
        .unwrap();
        merge_layer(&mut base, &layer);
        assert_eq!(base.gates.len(), 1);
        let remover = load_policy_str(
            r#"
schema_version = 1
[[gates]]
id = "t"
remove = true
"#,
        )
        .unwrap();
        merge_layer(&mut base, &remover);
        assert_eq!(base.gates.len(), 0);
    }
}
