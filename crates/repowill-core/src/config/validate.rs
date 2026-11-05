// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use repowill_model::Policy;
use std::collections::BTreeSet;

const KNOWN_STAGES: &[&str] = &[
    "pre-commit",
    "commit-msg",
    "post-commit",
    "pre-push",
    "pre-merge",
    "post-merge",
    "post-checkout",
    "pre-rebase",
    "agent-tool",
    "agent-stop",
    "ci",
    "manual",
];

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

pub fn validate(policy: &Policy) -> Vec<Diagnostic> {
    validate_with_source(policy, None)
}

pub fn validate_with_source(policy: &Policy, src: Option<&str>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let line_of = |needle: &str| -> Option<usize> {
        src.and_then(|s| {
            s.lines()
                .enumerate()
                .find(|(_, l)| l.contains(needle))
                .map(|(i, _)| i + 1)
        })
    };

    if policy.schema_version == 0 {
        diags.push(Diagnostic {
            code: "RW-E2001".into(),
            message: "schema_version must be >= 1".into(),
            line: None,
            column: None,
        });
    }

    // gates
    let mut seen = BTreeSet::new();
    for g in &policy.gates {
        if g.id.is_empty() {
            diags.push(Diagnostic {
                code: "RW-E2002".into(),
                message: "gate id must not be empty".into(),
                line: line_of(&g.name),
                column: None,
            });
        } else if !seen.insert(g.id.clone()) {
            diags.push(Diagnostic {
                code: "RW-E2003".into(),
                message: format!("duplicate gate id: {}", g.id),
                line: line_of(&g.id),
                column: None,
            });
        }
        if g.stages.is_empty() {
            diags.push(Diagnostic {
                code: "RW-E2004".into(),
                message: format!("gate {} has no stages", g.id),
                line: line_of(&g.id),
                column: None,
            });
        } else {
            for st in &g.stages {
                if !KNOWN_STAGES.contains(&st.as_str()) {
                    diags.push(Diagnostic {
                        code: "RW-E2014".into(),
                        message: format!("gate {} has unknown stage {}", g.id, st),
                        line: line_of(st),
                        column: None,
                    });
                }
            }
        }
        // glob compile check for paths
        for inc in &g.paths.include {
            if globset::Glob::new(inc).is_err() {
                diags.push(Diagnostic {
                    code: "RW-E2005".into(),
                    message: format!("invalid glob in gate {}: {}", g.id, inc),
                    line: line_of(inc),
                    column: None,
                });
            }
        }
    }

    // waivers
    let mut wseen = BTreeSet::new();
    for w in &policy.waivers {
        if w.id.is_empty() {
            diags.push(Diagnostic {
                code: "RW-E2006".into(),
                message: "waiver id must not be empty".into(),
                line: None,
                column: None,
            });
        } else if !wseen.insert(w.id.clone()) {
            diags.push(Diagnostic {
                code: "RW-E2007".into(),
                message: format!("duplicate waiver id: {}", w.id),
                line: line_of(&w.id),
                column: None,
            });
        }
    }

    // protected_paths unique
    let mut pseen = BTreeSet::new();
    for pp in &policy.protection.protected_paths {
        if !pp.id.is_empty() && !pseen.insert(pp.id.clone()) {
            diags.push(Diagnostic {
                code: "RW-E2008".into(),
                message: format!("duplicate protected_path id: {}", pp.id),
                line: None,
                column: None,
            });
        }
    }

    // docs.coupling unique
    let mut cseen = BTreeSet::new();
    for c in &policy.docs.coupling {
        if !c.id.is_empty() && !cseen.insert(c.id.clone()) {
            diags.push(Diagnostic {
                code: "RW-E2009".into(),
                message: format!("duplicate coupling id: {}", c.id),
                line: None,
                column: None,
            });
        }
    }

    // docs.generated unique
    let mut gseen = BTreeSet::new();
    for g in &policy.docs.generated {
        if !g.id.is_empty() && !gseen.insert(g.id.clone()) {
            diags.push(Diagnostic {
                code: "RW-E2010".into(),
                message: format!("duplicate generated doc id: {}", g.id),
                line: None,
                column: None,
            });
        }
    }

    // dependencies.exceptions unique
    let mut eseen = BTreeSet::new();
    for e in &policy.dependencies.exceptions {
        if !e.id.is_empty() && !eseen.insert(e.id.clone()) {
            diags.push(Diagnostic {
                code: "RW-E2011".into(),
                message: format!("duplicate dependency exception id: {}", e.id),
                line: None,
                column: None,
            });
        }
    }

    // identity.allowed unique
    let mut iseen = BTreeSet::new();
    for e in &policy.identity.allowed {
        if !e.id.is_empty() && !iseen.insert(e.id.clone()) {
            diags.push(Diagnostic {
                code: "RW-E2012".into(),
                message: format!("duplicate identity entry id: {}", e.id),
                line: None,
                column: None,
            });
        }
    }

    if policy.min_repowill_version.is_empty() {
        diags.push(Diagnostic {
            code: "RW-E2013".into(),
            message: "min_repowill_version must not be empty".into(),
            line: None,
            column: None,
        });
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::Policy;

    #[test]
    fn valid_default() {
        assert!(validate(&Policy::default()).is_empty());
    }

    #[test]
    fn duplicate_gate_id() {
        let mut p = Policy::default();
        for name in ["X", "X2"] {
            p.gates.push(repowill_model::Gate {
                id: "x".into(),
                name: name.into(),
                description: String::new(),
                uses: "builtin:x".into(),
                with: serde_json::Value::Null,
                stages: vec!["pre-commit".into()],
                mode: repowill_model::Mode::Block,
                stage_modes: std::collections::BTreeMap::new(),
                trigger: "auto".into(),
                on_error: "block".into(),
                timeout: "10m".into(),
                paths: repowill_model::GatePaths {
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
            });
        }
        assert!(!validate(&p).is_empty());
    }

    #[test]
    fn invalid_glob() {
        let mut p = Policy::default();
        p.gates.push(repowill_model::Gate {
            id: "g".into(),
            name: "G".into(),
            description: String::new(),
            uses: "builtin:g".into(),
            with: serde_json::Value::Null,
            stages: vec!["pre-commit".into()],
            mode: repowill_model::Mode::Block,
            stage_modes: std::collections::BTreeMap::new(),
            trigger: "auto".into(),
            on_error: "block".into(),
            timeout: "10m".into(),
            paths: repowill_model::GatePaths {
                include: vec!["[".into()],
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
        });
        assert!(!validate(&p).is_empty());
    }
}
