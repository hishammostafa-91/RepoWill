// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Semantic policy validation.
//!
//! Validation is deliberately fail closed: every rule in
//! `plan/03-POLICY-AND-DATA.md` section 3 and the gate catalog is checked,
//! and diagnostics carry a stable code, severity, rule, optional fix hint,
//! and line/column when the original source is available.

use crate::config::ENGINE_VERSION;
use crate::config::migrate::SUPPORTED_SCHEMA_VERSION;
use crate::config::value::{policy_to_value, resolve_path};
use repowill_model::{
    Gate, Mode, Policy, ProtectedMode, SigningMode, WeakeningMode, WorkflowModel,
};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// Diagnostic severity. Errors block loading/editing; warnings are shown but
/// do not change the policy outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

/// A single validation finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    /// Dotted policy path using array indexes (for example
    /// `gates.0.paths.include.1`).
    pub path: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub rule: Option<String>,
    pub fix: Option<String>,
}

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

const GUARD_NAMES: &[&str] = &[
    "main_protection",
    "branch_origin",
    "branch_naming",
    "history_protection",
    "delete_guard",
    "protected_paths",
    "policy_guard",
    "identity",
    "merge_guard",
    "push_guard",
    "agent_guard",
];

const SEVERITIES: &[&str] = &["none", "low", "medium", "high", "critical"];
const MODE_STRINGS: &[&str] = &["block", "warn", "off"];

/// Validate a policy without source text.
pub fn validate(policy: &Policy) -> Vec<Diagnostic> {
    validate_for_version(policy, None, ENGINE_VERSION)
}

/// Validate a policy against the engine contract version.
pub fn validate_with_source(policy: &Policy, source: Option<&str>) -> Vec<Diagnostic> {
    validate_for_version(policy, source, ENGINE_VERSION)
}

/// Validate a policy document, reporting TOML/schema errors as diagnostics.
pub fn validate_source(source: &str) -> Vec<Diagnostic> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    match parse_policy(source) {
        Ok(policy) => validate_with_source(&policy, Some(source)),
        Err(error) => vec![parse_diagnostic(source, &error)],
    }
}

fn parse_policy(source: &str) -> Result<Policy, toml::de::Error> {
    toml::from_str(source)
}

fn parse_diagnostic(source: &str, error: &toml::de::Error) -> Diagnostic {
    let (line, column) = error
        .span()
        .map(|span| line_col(source, span.start))
        .unwrap_or((None, None));
    let message = error.message().to_string();
    let fix = unknown_token_suggestion(&message);
    Diagnostic {
        code: "RW-E2000".to_string(),
        severity: Severity::Error,
        message,
        path: None,
        line,
        column,
        rule: Some("policy-schema".to_string()),
        fix,
    }
}

/// Suggest the closest declared key or enum variant for an unknown-token
/// deserialization error.
fn unknown_token_suggestion(message: &str) -> Option<String> {
    let (_, rest) = ["unknown field `", "unknown variant `"]
        .iter()
        .find_map(|marker| message.split_once(marker))?;
    let (token, after) = rest.split_once('`')?;
    let expected = after.split_once("expected")?.1;
    let candidates: Vec<&str> = expected
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|candidate| !candidate.is_empty())
        .collect();
    let best = candidates
        .into_iter()
        .map(|candidate| (edit_distance(token, candidate), candidate))
        .filter(|(distance, _)| *distance <= 2 || *distance * 3 <= token.len().max(3))
        .min_by_key(|(distance, candidate)| (*distance, *candidate))?;
    Some(format!("did you mean `{}`?", best.1))
}

fn edit_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (i, a) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, b) in right.iter().enumerate() {
            let substitution = previous[j] + usize::from(a != b);
            current[j + 1] = (previous[j + 1] + 1).min(current[j] + 1).min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// Validate a policy with an explicit running RepoWill version.
///
/// `running` is compared against `min_repowill_version`; a policy that
/// requires a newer binary is rejected (fail closed).
pub fn validate_for_version(
    policy: &Policy,
    source: Option<&str>,
    running: &str,
) -> Vec<Diagnostic> {
    let source = source.map(|text| text.strip_prefix('\u{feff}').unwrap_or(text));
    let locator = source.and_then(Locator::new);
    let mut ctx = Ctx {
        locator: locator.as_ref(),
        diags: Vec::new(),
    };

    check_schema_version(&mut ctx, policy);
    check_min_version(&mut ctx, policy, running);
    check_project(&mut ctx, policy);
    check_identity(&mut ctx, policy);
    check_workflow(&mut ctx, policy);
    check_protection(&mut ctx, policy);
    check_dependencies(&mut ctx, policy);
    check_docs(&mut ctx, policy);
    check_agents(&mut ctx, policy);
    check_remote(&mut ctx, policy);
    check_engine(&mut ctx, policy);
    check_gates(&mut ctx, policy);
    check_waivers(&mut ctx, policy);
    check_lock(&mut ctx, policy);
    check_extends(&mut ctx, policy);
    check_guards(&mut ctx, policy);

    let mut seen = BTreeSet::new();
    ctx.diags.retain(|diagnostic| {
        seen.insert((
            diagnostic.code.clone(),
            diagnostic.message.clone(),
            diagnostic.path.clone(),
            diagnostic.line,
            diagnostic.column,
        ))
    });
    ctx.diags
}

struct Ctx<'a> {
    locator: Option<&'a Locator>,
    diags: Vec<Diagnostic>,
}

impl Ctx<'_> {
    fn push(
        &mut self,
        code: &str,
        severity: Severity,
        message: String,
        path: Option<String>,
        rule: &str,
        fix: Option<String>,
    ) {
        let position = path.as_deref().and_then(|path| {
            self.locator.and_then(|locator| {
                locator
                    .span_at(path)
                    .map(|span| locator.line_col(span.start))
            })
        });
        let (line, column) = position.unwrap_or((None, None));
        self.diags.push(Diagnostic {
            code: code.to_string(),
            severity,
            message,
            path,
            line,
            column,
            rule: Some(rule.to_string()),
            fix,
        });
    }

    fn error(&mut self, code: &str, message: impl Into<String>, path: &str, rule: &str) {
        self.push(
            code,
            Severity::Error,
            message.into(),
            Some(path.to_string()),
            rule,
            None,
        );
    }

    fn error_with_fix(
        &mut self,
        code: &str,
        message: impl Into<String>,
        path: &str,
        rule: &str,
        fix: impl Into<String>,
    ) {
        self.push(
            code,
            Severity::Error,
            message.into(),
            Some(path.to_string()),
            rule,
            Some(fix.into()),
        );
    }

    fn warning(&mut self, code: &str, message: impl Into<String>, path: &str, rule: &str) {
        self.push(
            code,
            Severity::Warning,
            message.into(),
            Some(path.to_string()),
            rule,
            None,
        );
    }
}

// ---------------------------------------------------------------------------
// Source locator
// ---------------------------------------------------------------------------

struct Locator {
    source: String,
    document: toml_edit::Document<String>,
}

impl Locator {
    fn new(source: &str) -> Option<Self> {
        let document = toml_edit::Document::parse(source.to_string()).ok()?;
        Some(Self {
            source: source.to_string(),
            document,
        })
    }

    fn span_at(&self, path: &str) -> Option<Range<usize>> {
        let segments: Vec<&str> = path.split('.').collect();
        walk_item(self.document.as_item(), &segments)
    }

    fn line_col(&self, offset: usize) -> (Option<usize>, Option<usize>) {
        line_col(&self.source, offset)
    }
}

fn line_col(source: &str, offset: usize) -> (Option<usize>, Option<usize>) {
    if offset > source.len() {
        return (None, None);
    }
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rfind('\n')
        .map(|index| prefix[index + 1..].chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);
    (Some(line), Some(column))
}

fn walk_item(item: &toml_edit::Item, segments: &[&str]) -> Option<Range<usize>> {
    let Some((segment, rest)) = segments.split_first() else {
        return item.span();
    };
    match item {
        toml_edit::Item::Table(table) => walk_table(table, segment, rest),
        toml_edit::Item::ArrayOfTables(array) => walk_array_of_tables(array, segment, rest),
        toml_edit::Item::Value(value) => walk_value(value, segment, rest),
        toml_edit::Item::None => None,
    }
}

fn walk_table(table: &toml_edit::Table, segment: &str, rest: &[&str]) -> Option<Range<usize>> {
    let (key, item) = table.get_key_value(segment)?;
    if rest.is_empty() {
        item.span().or_else(|| key.span())
    } else {
        walk_item(item, rest)
    }
}

fn walk_array_of_tables(
    array: &toml_edit::ArrayOfTables,
    segment: &str,
    rest: &[&str],
) -> Option<Range<usize>> {
    let table = if let Ok(index) = segment.parse::<usize>() {
        array.get(index)?
    } else {
        array
            .iter()
            .find(|table| table.get("id").and_then(|value| value.as_str()) == Some(segment))?
    };
    if rest.is_empty() {
        table.span()
    } else {
        walk_table(table, rest[0], &rest[1..])
    }
}

fn walk_value(value: &toml_edit::Value, segment: &str, rest: &[&str]) -> Option<Range<usize>> {
    match value {
        toml_edit::Value::InlineTable(table) => {
            let (key, item) = table.get_key_value(segment)?;
            if rest.is_empty() {
                item.span().or_else(|| key.span())
            } else {
                walk_item(item, rest)
            }
        }
        toml_edit::Value::Array(array) => {
            let value = if let Ok(index) = segment.parse::<usize>() {
                array.get(index)?
            } else {
                array.iter().find(|value| {
                    value
                        .as_inline_table()
                        .and_then(|table| table.get("id"))
                        .and_then(toml_edit::Value::as_str)
                        == Some(segment)
                })?
            };
            if rest.is_empty() {
                value.span()
            } else {
                walk_value(value, rest[0], &rest[1..])
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Generic helpers
// ---------------------------------------------------------------------------

fn check_schema_version(ctx: &mut Ctx, policy: &Policy) {
    if policy.schema_version > SUPPORTED_SCHEMA_VERSION {
        ctx.error_with_fix(
            "RW-E2001",
            format!(
                "policy schema version {} is newer than the supported version {}",
                policy.schema_version, SUPPORTED_SCHEMA_VERSION
            ),
            "schema_version",
            "schema-version",
            "update RepoWill to a release that supports this schema",
        );
    } else if policy.schema_version < 1 {
        ctx.error(
            "RW-E2001",
            "schema_version must be at least 1",
            "schema_version",
            "schema-version",
        );
    } else if policy.schema_version < SUPPORTED_SCHEMA_VERSION {
        ctx.error_with_fix(
            "RW-E2001",
            format!(
                "policy schema version {} needs a migration to version {}",
                policy.schema_version, SUPPORTED_SCHEMA_VERSION
            ),
            "schema_version",
            "schema-version",
            "run `repowill policy migrate`",
        );
    }
}

fn check_min_version(ctx: &mut Ctx, policy: &Policy, running: &str) {
    let minimum = policy.min_repowill_version.trim();
    if minimum.is_empty() {
        ctx.error_with_fix(
            "RW-E2013",
            "min_repowill_version must not be empty",
            "min_repowill_version",
            "min-repowill-version",
            "use a semantic version such as \"1.0.0\"",
        );
        return;
    }
    let Some(minimum_version) = Semver::parse(minimum) else {
        ctx.error_with_fix(
            "RW-E2013",
            format!("min_repowill_version `{minimum}` is not a semantic version"),
            "min_repowill_version",
            "min-repowill-version",
            "use MAJOR.MINOR.PATCH, for example \"1.0.0\"",
        );
        return;
    };
    let Some(current) = Semver::parse(running) else {
        return;
    };
    if minimum_version > current {
        ctx.error_with_fix(
            "RW-E2018",
            format!("policy requires RepoWill {minimum} or newer but this binary is {running}"),
            "min_repowill_version",
            "min-repowill-version",
            format!("install RepoWill {minimum} or newer"),
        );
    }
}

fn check_project(ctx: &mut Ctx, policy: &Policy) {
    const PRESETS: &[&str] = &["starter", "recommended", "strict", "maximum", "custom"];
    const STACKS: &[&str] = &["node", "python", "rust", "go", "dotnet", "java", "docs"];
    check_enum(
        ctx,
        &policy.project.preset,
        "project.preset",
        PRESETS,
        "project preset",
    );
    for (index, stack) in policy.project.stacks.iter().enumerate() {
        check_enum(
            ctx,
            stack,
            &format!("project.stacks.{index}"),
            STACKS,
            "project stack",
        );
    }
}

fn check_identity(ctx: &mut Ctx, policy: &Policy) {
    let allowed = &policy.identity.allowed;
    check_unique_ids(
        ctx,
        allowed,
        "identity.allowed",
        "identity entry",
        "RW-E2012",
        "RW-E2012",
        "unique-id",
        false,
    );
    for (index, entry) in allowed.iter().enumerate() {
        let path = format!("identity.allowed.{index}");
        if entry.name.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                "identity.allowed entry needs a non-empty name",
                &format!("{path}.name"),
                "identity",
            );
        }
        if entry.email.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                "identity.allowed entry needs a non-empty email",
                &format!("{path}.email"),
                "identity",
            );
        } else if !looks_like_email(&entry.email) {
            ctx.error(
                "RW-E2035",
                format!("`{}` is not a valid email address", entry.email),
                &format!("{path}.email"),
                "identity",
            );
        }
    }
    if !policy.identity.email.is_empty() && !looks_like_email(&policy.identity.email) {
        ctx.error(
            "RW-E2035",
            format!(
                "`{}` is not a valid identity email address",
                policy.identity.email
            ),
            "identity.email",
            "identity",
        );
    }
    let _: SigningMode = policy.identity.signing;
}

fn check_guards(_ctx: &mut Ctx, policy: &Policy) {
    let _: Mode = policy.guards.main_protection;
    // The guard mirror of branch_naming.mode is intentionally not forced to
    // be equal: either may be stricter, and weakening detection covers drift.
    let _ = GUARD_NAMES;
}

fn check_workflow(ctx: &mut Ctx, policy: &Policy) {
    let workflow = &policy.workflow;
    let _: WorkflowModel = workflow.model;
    check_branch_name(ctx, &workflow.main_branch, "workflow.main_branch");

    if !workflow.protected_branches.is_empty() {
        for (index, branch) in workflow.protected_branches.iter().enumerate() {
            check_branch_name(ctx, branch, &format!("workflow.protected_branches.{index}"));
        }
        if !workflow
            .protected_branches
            .iter()
            .any(|branch| branch == &workflow.main_branch)
            && policy.guards.main_protection != Mode::Off
        {
            ctx.warning(
                "RW-E2026",
                format!(
                    "main_branch `{}` is not listed in protected_branches",
                    workflow.main_branch
                ),
                "workflow.protected_branches",
                "workflow",
            );
        }
    }
    if !workflow.integration_branch.is_empty() {
        check_branch_name(
            ctx,
            &workflow.integration_branch,
            "workflow.integration_branch",
        );
    }
    for (index, branch) in workflow.allowed_bases.iter().enumerate() {
        check_branch_name(ctx, branch, &format!("workflow.allowed_bases.{index}"));
    }
    for (branch_type, bases) in &workflow.bases_by_type {
        if !is_valid_branch_type(branch_type) {
            ctx.error(
                "RW-E2030",
                format!("`{branch_type}` is not a valid branch type"),
                &format!("workflow.bases_by_type.{branch_type}"),
                "workflow",
            );
        }
        for (index, branch) in bases.iter().enumerate() {
            check_branch_name(
                ctx,
                branch,
                &format!("workflow.bases_by_type.{branch_type}.{index}"),
            );
        }
    }
    if workflow.stale_branch_days < 0 {
        ctx.error(
            "RW-E2019",
            "workflow.stale_branch_days must be >= 0",
            "workflow.stale_branch_days",
            "workflow",
        );
    }
    if workflow.max_commits_ahead < 0 {
        ctx.error(
            "RW-E2019",
            "workflow.max_commits_ahead must be >= 0",
            "workflow.max_commits_ahead",
            "workflow",
        );
    }
    for (index, pattern) in workflow.stacked_patterns.iter().enumerate() {
        check_glob(
            ctx,
            pattern,
            &format!("workflow.stacked_patterns.{index}"),
            "workflow",
        );
    }

    let naming = &workflow.branch_naming;
    if naming.types.is_empty() {
        ctx.error(
            "RW-E2034",
            "workflow.branch_naming.types must not be empty",
            "workflow.branch_naming.types",
            "workflow",
        );
    }
    for (index, branch_type) in naming.types.iter().enumerate() {
        if !is_valid_branch_type(branch_type) {
            ctx.error(
                "RW-E2030",
                format!("`{branch_type}` is not a valid branch type"),
                &format!("workflow.branch_naming.types.{index}"),
                "workflow",
            );
        }
    }
    if !naming.pattern.is_empty() {
        check_regex(
            ctx,
            &naming.pattern,
            "workflow.branch_naming.pattern",
            "workflow",
        );
    }
    for (index, pattern) in naming.exceptions.iter().enumerate() {
        check_glob(
            ctx,
            pattern,
            &format!("workflow.branch_naming.exceptions.{index}"),
            "workflow",
        );
    }

    check_commits(ctx, policy);
    check_merge(ctx, policy);
    check_push(ctx, policy);

    let tags = &workflow.tags;
    if tags.pattern.is_empty() {
        ctx.error_with_fix(
            "RW-E2031",
            "workflow.tags.pattern must not be empty",
            "workflow.tags.pattern",
            "workflow",
            "use the default `^v(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)$`",
        );
    } else {
        check_regex(ctx, &tags.pattern, "workflow.tags.pattern", "workflow");
    }
    for (index, branch) in tags.only_on.iter().enumerate() {
        check_branch_name(ctx, branch, &format!("workflow.tags.only_on.{index}"));
    }
}

fn check_commits(ctx: &mut Ctx, policy: &Policy) {
    let commits = &policy.workflow.commits;
    if commits.types.is_empty() {
        ctx.error(
            "RW-E2034",
            "workflow.commits.types must not be empty",
            "workflow.commits.types",
            "workflow",
        );
    }
    for (index, commit_type) in commits.types.iter().enumerate() {
        if !is_valid_branch_type(commit_type) {
            ctx.error(
                "RW-E2030",
                format!("`{commit_type}` is not a valid commit type"),
                &format!("workflow.commits.types.{index}"),
                "workflow",
            );
        }
    }
    if commits.subject_max <= 0 {
        ctx.error(
            "RW-E2019",
            "workflow.commits.subject_max must be positive",
            "workflow.commits.subject_max",
            "workflow",
        );
    }
    if commits.body_max_line < 0 {
        ctx.error(
            "RW-E2019",
            "workflow.commits.body_max_line must be >= 0",
            "workflow.commits.body_max_line",
            "workflow",
        );
    }
    if commits.atomic_max_files <= 0 {
        ctx.error(
            "RW-E2019",
            "workflow.commits.atomic_max_files must be positive",
            "workflow.commits.atomic_max_files",
            "workflow",
        );
    }
    if commits.atomic_max_lines <= 0 {
        ctx.error(
            "RW-E2019",
            "workflow.commits.atomic_max_lines must be positive",
            "workflow.commits.atomic_max_lines",
            "workflow",
        );
    }
    for (index, trailer) in commits.required_trailers.iter().enumerate() {
        if trailer.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                "required trailer must not be empty",
                &format!("workflow.commits.required_trailers.{index}"),
                "workflow",
            );
        }
    }
    for (glob, scope) in &commits.scope_map {
        check_glob(
            ctx,
            glob,
            &format!("workflow.commits.scope_map.{glob}"),
            "workflow",
        );
        if scope.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                format!("scope_map entry for `{glob}` has an empty scope"),
                &format!("workflow.commits.scope_map.{glob}"),
                "workflow",
            );
        }
    }
    if !commits.custom_pattern.is_empty() {
        check_regex(
            ctx,
            &commits.custom_pattern,
            "workflow.commits.custom_pattern",
            "workflow",
        );
    }
    if commits.convention == repowill_model::Convention::Custom && commits.custom_pattern.is_empty()
    {
        ctx.error_with_fix(
            "RW-E2015",
            "convention = \"custom\" requires custom_pattern",
            "workflow.commits.custom_pattern",
            "workflow",
            "set `custom_pattern` to a regular expression",
        );
    }
    if !commits.issue_pattern.is_empty() {
        check_regex(
            ctx,
            &commits.issue_pattern,
            "workflow.commits.issue_pattern",
            "workflow",
        );
    }
    if commits.require_issue_ref && commits.issue_pattern.is_empty() {
        ctx.error(
            "RW-E2016",
            "require_issue_ref = true requires a non-empty issue_pattern",
            "workflow.commits.issue_pattern",
            "workflow",
        );
    }
}

fn check_merge(ctx: &mut Ctx, policy: &Policy) {
    let merge = &policy.workflow.merge;
    if merge.approval_expiry_hours <= 0 {
        ctx.error(
            "RW-E2019",
            "workflow.merge.approval_expiry_hours must be positive",
            "workflow.merge.approval_expiry_hours",
            "workflow",
        );
    }
    if let Err(message) = check_template(&merge.message_template) {
        ctx.error_with_fix(
            "RW-E2027",
            message,
            "workflow.merge.message_template",
            "workflow",
            "allowed placeholders: {branch} {target} {request} {title}",
        );
    }
}

fn check_template(template: &str) -> Result<(), String> {
    const ALLOWED: &[&str] = &["branch", "target", "request", "title"];
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return Err("message_template has an unclosed `{`".to_string());
        };
        let name = &after[..end];
        if !ALLOWED.contains(&name) {
            return Err(format!(
                "message_template uses unknown placeholder `{{{name}}}`"
            ));
        }
        rest = &after[end + 1..];
    }
    if rest.contains('}') {
        return Err("message_template has an unmatched `}`".to_string());
    }
    Ok(())
}

fn check_push(_ctx: &mut Ctx, policy: &Policy) {
    let push = &policy.workflow.push;
    let _ = push.allow_force;
    let _ = push.protected_direct;
    let _ = push.remote_deletions;
    let _: Mode = push.verified_commits;
}

fn check_protection(ctx: &mut Ctx, policy: &Policy) {
    let protection = &policy.protection;
    check_unique_ids(
        ctx,
        &protection.protected_paths,
        "protection.protected_paths",
        "protected path",
        "RW-E2008",
        "RW-E2008",
        "unique-id",
        false,
    );
    for (index, entry) in protection.protected_paths.iter().enumerate() {
        let path = format!("protection.protected_paths.{index}");
        if entry.pattern.trim().is_empty() {
            ctx.error(
                "RW-E2022",
                "protected path pattern must not be empty",
                &format!("{path}.pattern"),
                "protection",
            );
        } else {
            check_glob(
                ctx,
                &entry.pattern,
                &format!("{path}.pattern"),
                "protection",
            );
        }
        if entry.reason.trim().is_empty() {
            ctx.warning(
                "RW-E2034",
                "protected path should explain why in `reason`",
                &format!("{path}.reason"),
                "protection",
            );
        }
        let _: ProtectedMode = entry.mode;
    }

    let delete = &protection.delete;
    for (name, value) in [
        ("files", &delete.files),
        ("branches", &delete.branches),
        ("tags", &delete.tags),
    ] {
        check_enum(
            ctx,
            value,
            &format!("protection.delete.{name}"),
            &["approval", "warn", "off"],
            "deletion mode",
        );
    }
    for (index, pattern) in delete.allow_patterns.iter().enumerate() {
        check_glob(
            ctx,
            pattern,
            &format!("protection.delete.allow_patterns.{index}"),
            "protection",
        );
    }
    let _: WeakeningMode = protection.policy.weakening;
    let snapshots = &protection.snapshots;
    if snapshots.interval_minutes <= 0 {
        ctx.error(
            "RW-E2019",
            "protection.snapshots.interval_minutes must be positive",
            "protection.snapshots.interval_minutes",
            "protection",
        );
    }
    if snapshots.max_file_size_mb <= 0 {
        ctx.error(
            "RW-E2019",
            "protection.snapshots.max_file_size_mb must be positive",
            "protection.snapshots.max_file_size_mb",
            "protection",
        );
    }
    if snapshots.retention_days < 0 {
        ctx.error(
            "RW-E2019",
            "protection.snapshots.retention_days must be >= 0",
            "protection.snapshots.retention_days",
            "protection",
        );
    }
    if snapshots.max_count <= 0 {
        ctx.error(
            "RW-E2019",
            "protection.snapshots.max_count must be positive",
            "protection.snapshots.max_count",
            "protection",
        );
    }
}

fn check_dependencies(ctx: &mut Ctx, policy: &Policy) {
    let dependencies = &policy.dependencies;
    check_enum(
        ctx,
        &dependencies.scope,
        "dependencies.scope",
        &["new-and-changed", "all"],
        "dependency scope",
    );
    let vulnerabilities = &dependencies.vulnerabilities;
    for (name, value) in [
        ("block_at", &vulnerabilities.block_at),
        ("warn_at", &vulnerabilities.warn_at),
    ] {
        check_enum(
            ctx,
            value,
            &format!("dependencies.vulnerabilities.{name}"),
            SEVERITIES,
            "severity",
        );
    }
    check_enum(
        ctx,
        &vulnerabilities.unknown_severity,
        "dependencies.vulnerabilities.unknown_severity",
        &["block", "warn", "ignore"],
        "unknown severity mode",
    );

    let licenses = &dependencies.licenses;
    check_enum(
        ctx,
        &licenses.unknown,
        "dependencies.licenses.unknown",
        &["block", "warn"],
        "unknown license mode",
    );
    for (field, values) in [
        ("allowed", &licenses.allowed),
        ("content_licenses", &licenses.content_licenses),
        ("denied", &licenses.denied),
    ] {
        for (index, license) in values.iter().enumerate() {
            if license.trim().is_empty() {
                ctx.error(
                    "RW-E2034",
                    "license entries must not be empty",
                    &format!("dependencies.licenses.{field}.{index}"),
                    "dependencies",
                );
            }
        }
    }

    let versions = &dependencies.versions;
    check_enum(
        ctx,
        &versions.applies_to,
        "dependencies.versions.applies_to",
        &["direct", "all"],
        "version scope",
    );
    check_enum(
        ctx,
        &versions.tolerance,
        "dependencies.versions.tolerance",
        &["exact", "patch", "minor", "major"],
        "version tolerance",
    );
    check_enum(
        ctx,
        &versions.outdated_existing,
        "dependencies.versions.outdated_existing",
        &["off", "warn", "block"],
        "outdated dependency mode",
    );
    if versions.min_release_age_days < 0 {
        ctx.error(
            "RW-E2019",
            "min_release_age_days must be >= 0",
            "dependencies.versions.min_release_age_days",
            "dependencies",
        );
    }
    if versions.outdated_grace_days < 0 {
        ctx.error(
            "RW-E2019",
            "outdated_grace_days must be >= 0",
            "dependencies.versions.outdated_grace_days",
            "dependencies",
        );
    }

    for (index, registry) in dependencies.sources.allowed_registries.iter().enumerate() {
        if registry.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                "allowed_registries entries must not be empty",
                &format!("dependencies.sources.allowed_registries.{index}"),
                "dependencies",
            );
        }
    }

    check_enum(
        ctx,
        &dependencies.risk.install_scripts,
        "dependencies.risk.install_scripts",
        &["block", "warn", "off"],
        "install script mode",
    );
    if dependencies.risk.unmaintained_days < 0 {
        ctx.error(
            "RW-E2019",
            "unmaintained_days must be >= 0",
            "dependencies.risk.unmaintained_days",
            "dependencies",
        );
    }
    check_enum(
        ctx,
        &dependencies.network.mode,
        "dependencies.network.mode",
        &["online", "offline", "cache-only"],
        "network mode",
    );
    check_enum(
        ctx,
        &dependencies.network.on_unavailable,
        "dependencies.network.on_unavailable",
        &["block", "warn"],
        "offline fallback mode",
    );
    if dependencies.network.cache_ttl_hours < 0 {
        ctx.error(
            "RW-E2019",
            "cache_ttl_hours must be >= 0",
            "dependencies.network.cache_ttl_hours",
            "dependencies",
        );
    }

    check_unique_ids(
        ctx,
        &dependencies.exceptions,
        "dependencies.exceptions",
        "dependency exception",
        "RW-E2011",
        "RW-E2011",
        "unique-id",
        false,
    );
    for (index, exception) in dependencies.exceptions.iter().enumerate() {
        let path = format!("dependencies.exceptions.{index}");
        if exception.package.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                "dependency exception needs a package",
                &format!("{path}.package"),
                "dependencies",
            );
        }
        if exception.versions.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                "dependency exception needs a version range",
                &format!("{path}.versions"),
                "dependencies",
            );
        }
        if exception.reason.trim().is_empty() {
            ctx.warning(
                "RW-E2034",
                "dependency exception should explain why in `reason`",
                &format!("{path}.reason"),
                "dependencies",
            );
        }
        check_date(ctx, &exception.expires, &format!("{path}.expires"));
        for (allow_index, allow) in exception.allow.iter().enumerate() {
            let valid = matches!(allow.as_str(), "license" | "version" | "risk")
                || (allow.starts_with("vulnerability:") && allow.len() > 14);
            if !valid {
                ctx.error_with_fix(
                    "RW-E2030",
                    format!("`{allow}` is not a valid exception target"),
                    &format!("{path}.allow.{allow_index}"),
                    "dependencies",
                    "use license, version, risk, or vulnerability:<ID>",
                );
            }
        }
    }
}

fn check_docs(ctx: &mut Ctx, policy: &Policy) {
    let docs = &policy.docs;
    for (index, path) in docs.required.iter().enumerate() {
        check_repo_path(ctx, path, &format!("docs.required.{index}"), "docs");
    }
    if docs.staleness_days < 0 {
        ctx.error(
            "RW-E2019",
            "docs.staleness_days must be >= 0",
            "docs.staleness_days",
            "docs",
        );
    }
    if docs.staleness_key.trim().is_empty() {
        ctx.error(
            "RW-E2034",
            "docs.staleness_key must not be empty",
            "docs.staleness_key",
            "docs",
        );
    }
    check_unique_ids(
        ctx,
        &docs.coupling,
        "docs.coupling",
        "docs coupling rule",
        "RW-E2009",
        "RW-E2009",
        "unique-id",
        false,
    );
    for (index, rule) in docs.coupling.iter().enumerate() {
        let path = format!("docs.coupling.{index}");
        if rule.when_changed.is_empty() {
            ctx.error_with_fix(
                "RW-E2034",
                "docs coupling rule needs at least one `when_changed` glob",
                &format!("{path}.when_changed"),
                "docs",
                "for example when_changed = [\"src/**\"]",
            );
        }
        for (glob_index, pattern) in rule.when_changed.iter().enumerate() {
            check_glob(
                ctx,
                pattern,
                &format!("{path}.when_changed.{glob_index}"),
                "docs",
            );
        }
        for (glob_index, pattern) in rule.require_changed.iter().enumerate() {
            check_glob(
                ctx,
                pattern,
                &format!("{path}.require_changed.{glob_index}"),
                "docs",
            );
        }
        check_enum(
            ctx,
            &rule.mode,
            &format!("{path}.mode"),
            MODE_STRINGS,
            "docs mode",
        );
    }
    let changelog = &docs.changelog;
    check_repo_path(ctx, &changelog.file, "docs.changelog.file", "docs");
    if changelog.format.trim().is_empty() {
        ctx.error(
            "RW-E2034",
            "docs.changelog.format must not be empty",
            "docs.changelog.format",
            "docs",
        );
    }
    check_enum(
        ctx,
        &changelog.mode,
        "docs.changelog.mode",
        MODE_STRINGS,
        "changelog mode",
    );
    check_unique_ids(
        ctx,
        &docs.generated,
        "docs.generated",
        "generated document",
        "RW-E2010",
        "RW-E2010",
        "unique-id",
        false,
    );
    for (index, generated) in docs.generated.iter().enumerate() {
        let path = format!("docs.generated.{index}");
        if generated.command.trim().is_empty() {
            ctx.error_with_fix(
                "RW-E2034",
                "generated document needs a command",
                &format!("{path}.command"),
                "docs",
                "set `command` to the generator invocation",
            );
        }
        if generated.outputs.is_empty() {
            ctx.warning(
                "RW-E2034",
                "generated document has no outputs; the generator result cannot be checked",
                &format!("{path}.outputs"),
                "docs",
            );
        }
        for (output_index, output) in generated.outputs.iter().enumerate() {
            check_repo_path(
                ctx,
                output,
                &format!("{path}.outputs.{output_index}"),
                "docs",
            );
        }
    }
}

fn check_agents(ctx: &mut Ctx, policy: &Policy) {
    let agents = &policy.agents;
    const KNOWN_AGENTS: &[&str] = &["opencode", "claude-code", "generic"];
    for (index, agent) in agents.enabled.iter().enumerate() {
        if !KNOWN_AGENTS.contains(&agent.as_str()) {
            ctx.warning(
                "RW-E2030",
                format!("`{agent}` is not a known agent adapter"),
                &format!("agents.enabled.{index}"),
                "agents",
            );
        }
    }
    let guard = &agents.guard;
    for (index, pattern) in guard.deny_read.iter().enumerate() {
        check_glob(
            ctx,
            pattern,
            &format!("agents.guard.deny_read.{index}"),
            "agents",
        );
    }
    for (index, pattern) in guard.allow_read.iter().enumerate() {
        check_glob(
            ctx,
            pattern,
            &format!("agents.guard.allow_read.{index}"),
            "agents",
        );
    }
    check_enum(
        ctx,
        &guard.outside_workspace,
        "agents.guard.outside_workspace",
        &["deny", "ask"],
        "outside-workspace mode",
    );
    check_enum(
        ctx,
        &guard.unknown_command,
        "agents.guard.unknown_command",
        &["allow", "ask"],
        "unknown-command mode",
    );
    if agents.stop_check.gate_tags.is_empty() {
        ctx.warning(
            "RW-E2034",
            "agents.stop_check.gate_tags is empty; the stop check will run no gates",
            "agents.stop_check.gate_tags",
            "agents",
        );
    }
    if !policy.gates.is_empty() {
        let known_tags: BTreeSet<&str> = policy
            .gates
            .iter()
            .flat_map(|gate| gate.tags.iter().map(String::as_str))
            .collect();
        for (index, tag) in agents.stop_check.gate_tags.iter().enumerate() {
            if !known_tags.contains(tag.as_str()) {
                ctx.warning(
                    "RW-E2030",
                    format!("no gate uses the stop-check tag `{tag}`"),
                    &format!("agents.stop_check.gate_tags.{index}"),
                    "agents",
                );
            }
        }
    }
    if agents.stop_check.max_feedback_lines <= 0 {
        ctx.error(
            "RW-E2019",
            "agents.stop_check.max_feedback_lines must be positive",
            "agents.stop_check.max_feedback_lines",
            "agents",
        );
    }
    check_enum(
        ctx,
        &agents.parallel.mode,
        "agents.parallel.mode",
        &["worktrees", "forbid"],
        "parallel mode",
    );
}

fn check_remote(ctx: &mut Ctx, policy: &Policy) {
    let remote = &policy.remote;
    check_enum(
        ctx,
        &remote.provider,
        "remote.provider",
        &["none", "github"],
        "remote provider",
    );
    check_enum(
        ctx,
        &remote.visibility,
        "remote.visibility",
        &["public", "private"],
        "repository visibility",
    );
    if remote.remote_name.trim().is_empty() || remote.remote_name.contains(char::is_whitespace) {
        ctx.error(
            "RW-E2026",
            "remote.remote_name must be a valid git remote name",
            "remote.remote_name",
            "remote",
        );
    }
    check_enum(
        ctx,
        &remote.github.rulesets,
        "remote.github.rulesets",
        &["auto", "off"],
        "ruleset mode",
    );
    if remote.github.ci_workflow
        && (remote.github.ci_runs_on.trim().is_empty()
            || remote.github.required_check.trim().is_empty())
    {
        ctx.error(
            "RW-E2034",
            "github CI settings must not be empty when ci_workflow = true",
            "remote.github",
            "remote",
        );
    }
}

fn check_engine(ctx: &mut Ctx, policy: &Policy) {
    let engine = &policy.engine;
    if engine.concurrency < 0 {
        ctx.error(
            "RW-E2019",
            "engine.concurrency must be >= 0",
            "engine.concurrency",
            "engine",
        );
    }
    check_duration(
        ctx,
        &engine.default_timeout,
        "engine.default_timeout",
        "engine",
    );
    check_enum(
        ctx,
        &engine.default_on_error,
        "engine.default_on_error",
        &["block", "warn"],
        "engine error mode",
    );
}

fn check_gates(ctx: &mut Ctx, policy: &Policy) {
    check_unique_ids(
        ctx,
        &policy.gates,
        "gates",
        "gate",
        "RW-E2002",
        "RW-E2003",
        "unique-id",
        true,
    );

    for (index, gate) in policy.gates.iter().enumerate() {
        let path = format!("gates.{index}");
        check_gate(ctx, gate, &path);

        for (stage_index, stage) in gate.stages.iter().enumerate() {
            if !KNOWN_STAGES.contains(&stage.as_str()) {
                ctx.error_with_fix(
                    "RW-E2014",
                    format!("unknown stage `{stage}`"),
                    &format!("{path}.stages.{stage_index}"),
                    "gate-stage",
                    format!("known stages: {}", KNOWN_STAGES.join(", ")),
                );
            } else if stage == "agent-tool" {
                ctx.error_with_fix(
                    "RW-E2014",
                    "no gate kind supports the agent-tool stage in v1.0",
                    &format!("{path}.stages.{stage_index}"),
                    "gate-stage",
                    "use agent-stop, or rely on guard GD-11",
                );
            }
        }
        for (stage, mode) in &gate.stage_modes {
            if !KNOWN_STAGES.contains(&stage.as_str()) {
                ctx.error_with_fix(
                    "RW-E2014",
                    format!("stage_modes uses unknown stage `{stage}`"),
                    &format!("{path}.stage_modes.{stage}"),
                    "gate-stage",
                    format!("known stages: {}", KNOWN_STAGES.join(", ")),
                );
            } else if !gate.stages.contains(stage) {
                ctx.warning(
                    "RW-E2014",
                    format!("stage_modes overrides `{stage}` which is not listed in stages"),
                    &format!("{path}.stage_modes.{stage}"),
                    "gate-stage",
                );
            }
            let _: Mode = *mode;
        }

        for (include_index, pattern) in gate.paths.include.iter().enumerate() {
            check_glob(
                ctx,
                pattern,
                &format!("{path}.paths.include.{include_index}"),
                "gate-paths",
            );
        }
        for (exclude_index, pattern) in gate.paths.exclude.iter().enumerate() {
            check_glob(
                ctx,
                pattern,
                &format!("{path}.paths.exclude.{exclude_index}"),
                "gate-paths",
            );
        }
        for (branch_index, pattern) in gate.branches.iter().enumerate() {
            check_glob(
                ctx,
                pattern,
                &format!("{path}.branches.{branch_index}"),
                "gate-paths",
            );
        }
        for (os_index, os) in gate.os.iter().enumerate() {
            if !matches!(os.as_str(), "windows" | "linux") {
                ctx.error_with_fix(
                    "RW-E2030",
                    format!("unknown operating system `{os}`"),
                    &format!("{path}.os.{os_index}"),
                    "gate",
                    "use windows or linux; leave the list empty for all",
                );
            }
        }
    }

    check_gate_graph(ctx, policy);
}

fn check_gate(ctx: &mut Ctx, gate: &Gate, path: &str) {
    if gate.name.trim().is_empty() {
        ctx.error_with_fix(
            "RW-E2034",
            format!("gate `{}` needs a plain-language name", gate.id),
            &format!("{path}.name"),
            "gate-definition",
            "set `name` to a short description",
        );
    }
    check_gate_uses(ctx, gate, path);
    if gate.stages.is_empty() {
        ctx.error_with_fix(
            "RW-E2004",
            format!("gate `{}` has no stages", gate.id),
            &format!("{path}.stages"),
            "gate-stage",
            format!("use one or more of: {}", KNOWN_STAGES.join(", ")),
        );
    }
    check_enum(
        ctx,
        &gate.trigger,
        &format!("{path}.trigger"),
        &["auto", "manual"],
        "gate trigger",
    );
    check_enum(
        ctx,
        &gate.on_error,
        &format!("{path}.on_error"),
        &["block", "warn"],
        "gate error mode",
    );
    check_duration(ctx, &gate.timeout, &format!("{path}.timeout"), "gate");
    check_enum(
        ctx,
        &gate.only_in,
        &format!("{path}.only_in"),
        &["any", "local", "ci"],
        "gate location",
    );
    for (index, dependency) in gate.depends_on.iter().enumerate() {
        if dependency.trim().is_empty() {
            ctx.error(
                "RW-E2023",
                "depends_on entries must not be empty",
                &format!("{path}.depends_on.{index}"),
                "gate-dependency",
            );
        }
    }
    check_gate_with(ctx, gate, path);
}

const KNOWN_BUILTIN_KINDS: &[&str] = &[
    "commit-message",
    "atomic-commit",
    "signed-commits",
    "dco",
    "verified-commits",
    "large-files",
    "forbidden-files",
    "secrets",
    "conflict-markers",
    "portability",
    "text-hygiene",
    "format",
    "lint",
    "typecheck",
    "build",
    "tests",
    "coverage",
    "test-presence",
    "dep-vulnerabilities",
    "dep-licenses",
    "dep-versions",
    "dep-risk",
    "lockfile",
    "sarif",
    "workflow-security",
    "docs-required",
    "docs-links",
    "docs-lint",
    "docs-coupling",
    "docs-staleness",
    "changelog",
    "generated-sync",
];

fn check_gate_uses(ctx: &mut Ctx, gate: &Gate, path: &str) {
    if let Some(kind) = gate.uses.strip_prefix("builtin:") {
        if kind.is_empty() || !KNOWN_BUILTIN_KINDS.contains(&kind) {
            ctx.error_with_fix(
                "RW-E2017",
                format!("unknown builtin gate kind `{kind}`"),
                &format!("{path}.uses"),
                "gate-kind",
                format!("known builtin kinds: {}", KNOWN_BUILTIN_KINDS.join(", ")),
            );
        }
    } else if gate.uses != "command" && gate.uses != "script" {
        ctx.error_with_fix(
            "RW-E2017",
            format!("unknown gate kind `{}`", gate.uses),
            &format!("{path}.uses"),
            "gate-kind",
            "use `builtin:<kind>`, `command`, or `script`",
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamType {
    Bool,
    Int,
    Number,
    Text,
    TextList,
    IntList,
    Table,
}

#[derive(Clone, Copy)]
struct Param {
    name: &'static str,
    kind: ParamType,
    allowed: Option<&'static [&'static str]>,
}

const fn param(name: &'static str, kind: ParamType) -> Param {
    Param {
        name,
        kind,
        allowed: None,
    }
}

const fn param_of(name: &'static str, kind: ParamType, allowed: &'static [&'static str]) -> Param {
    Param {
        name,
        kind,
        allowed: Some(allowed),
    }
}

static TEXT_MODE: &[&str] = &["block", "warn", "off"];
static SHELLS: &[&str] = &["auto", "cmd", "pwsh", "powershell", "bash", "sh", "none"];
static PASS_FILES: &[&str] = &["none", "args", "stdin", "file"];
static OUTPUTS: &[&str] = &["text", "repowill-json", "sarif", "junit", "tap"];
static FILES_MODES: &[&str] = &["changed", "all"];
static SEVERITY_LIST: &[&str] = SEVERITIES;

static PARAMS_NONE: &[Param] = &[];
static PARAMS_COMMIT_MESSAGE: &[Param] = &[param_of("allow_fixup", ParamType::Text, TEXT_MODE)];
static PARAMS_ATOMIC: &[Param] = &[param("ignore", ParamType::TextList)];
static PARAMS_SIGNED: &[Param] = &[param("accept", ParamType::TextList)];
static PARAMS_LARGE_FILES: &[Param] = &[
    param("max_kb", ParamType::Int),
    param("lfs_ok", ParamType::Bool),
];
static PARAMS_FORBIDDEN: &[Param] = &[
    param("patterns", ParamType::TextList),
    param("allow", ParamType::TextList),
];
static PARAMS_SECRETS: &[Param] = &[
    param_of("tool", ParamType::Text, &["builtin", "gitleaks", "both"]),
    param("honor_inline_allow", ParamType::Bool),
    param("entropy_min", ParamType::Number),
];
static PARAMS_PORTABILITY: &[Param] = &[
    param("max_path", ParamType::Int),
    param_of("symlinks", ParamType::Text, TEXT_MODE),
];
static PARAMS_TEXT_HYGIENE: &[Param] = &[
    param_of("utf8", ParamType::Text, TEXT_MODE),
    param_of("bom", ParamType::Text, TEXT_MODE),
    param_of("crlf", ParamType::Text, TEXT_MODE),
    param_of("final_newline", ParamType::Text, TEXT_MODE),
    param_of("trailing_whitespace", ParamType::Text, TEXT_MODE),
];
static PARAMS_QUALITY: &[Param] = &[
    param("tool", ParamType::Text),
    param("command", ParamType::Text),
    param("fix_command", ParamType::Text),
    param_of("files_mode", ParamType::Text, FILES_MODES),
    param("output", ParamType::Text),
    param("cwd", ParamType::Text),
    param("env", ParamType::Table),
];
static PARAMS_LINT: &[Param] = &[
    param("tool", ParamType::Text),
    param("command", ParamType::Text),
    param("fix_command", ParamType::Text),
    param_of("files_mode", ParamType::Text, FILES_MODES),
    param("output", ParamType::Text),
    param("cwd", ParamType::Text),
    param("env", ParamType::Table),
    param("max_warnings", ParamType::Int),
];
static PARAMS_TESTS: &[Param] = &[
    param("tool", ParamType::Text),
    param("command", ParamType::Text),
    param("fix_command", ParamType::Text),
    param_of("files_mode", ParamType::Text, FILES_MODES),
    param("output", ParamType::Text),
    param("cwd", ParamType::Text),
    param("env", ParamType::Table),
    param("junit_report", ParamType::Text),
    param("retries", ParamType::Int),
];
static PARAMS_COVERAGE: &[Param] = &[
    param("tool", ParamType::Text),
    param("command", ParamType::Text),
    param("fix_command", ParamType::Text),
    param_of("files_mode", ParamType::Text, FILES_MODES),
    param("output", ParamType::Text),
    param("cwd", ParamType::Text),
    param("env", ParamType::Table),
    param("report", ParamType::Text),
    param_of(
        "format",
        ParamType::Text,
        &[
            "auto",
            "lcov",
            "cobertura",
            "jacoco",
            "go",
            "coveragepy-json",
        ],
    ),
    param("min_lines", ParamType::Number),
    param("min_branches", ParamType::Number),
    param("per_path", ParamType::TextList),
    param("exclude", ParamType::TextList),
    param("ratchet", ParamType::Bool),
    param("ratchet_tolerance", ParamType::Number),
];
static PARAMS_TEST_PRESENCE: &[Param] = &[
    param("source", ParamType::TextList),
    param("tests", ParamType::TextList),
    param("require_modified", ParamType::Bool),
];
static PARAMS_DEP_FULL: &[Param] = &[param("full", ParamType::Bool)];
static PARAMS_SARIF: &[Param] = &[
    param("tool", ParamType::Text),
    param("command", ParamType::Text),
    param("sarif_path", ParamType::Text),
    param_of("block_at", ParamType::Text, SEVERITY_LIST),
    param_of("warn_at", ParamType::Text, SEVERITY_LIST),
    param("only_changed_files", ParamType::Bool),
    param("baseline", ParamType::Text),
];
static PARAMS_WORKFLOW_SECURITY: &[Param] = &[param("require_sha_pins", ParamType::Bool)];
static PARAMS_DOCS_REQUIRED: &[Param] = &[param("min_bytes", ParamType::Int)];
static PARAMS_DOCS_LINKS: &[Param] = &[
    param("external", ParamType::Bool),
    param("include", ParamType::TextList),
];
static PARAMS_DOCS_LINT: &[Param] = &[param("tool", ParamType::Text)];
static PARAMS_DOCS_STALENESS: &[Param] = &[param("require_key", ParamType::Bool)];
static PARAMS_GENERATED_SYNC: &[Param] = &[param("ids", ParamType::TextList)];
static PARAMS_COMMAND: &[Param] = &[
    param("run", ParamType::Text),
    param("run_windows", ParamType::Text),
    param("run_linux", ParamType::Text),
    param_of("shell", ParamType::Text, SHELLS),
    param("cwd", ParamType::Text),
    param("env", ParamType::Table),
    param_of("pass_files", ParamType::Text, PASS_FILES),
    param_of("output", ParamType::Text, OUTPUTS),
    param("success_codes", ParamType::IntList),
    param("warn_codes", ParamType::IntList),
];
static PARAMS_SCRIPT: &[Param] = &[
    param("path", ParamType::Text),
    param("interpreter", ParamType::Text),
    param("args", ParamType::TextList),
    param_of("pass_files", ParamType::Text, PASS_FILES),
    param_of("output", ParamType::Text, OUTPUTS),
    param("success_codes", ParamType::IntList),
    param("warn_codes", ParamType::IntList),
];

fn params_for(gate: &Gate) -> Option<(&str, &'static [Param])> {
    match gate.uses.as_str() {
        "command" => Some(("command", PARAMS_COMMAND)),
        "script" => Some(("script", PARAMS_SCRIPT)),
        other => {
            let kind = other.strip_prefix("builtin:")?;
            Some((
                kind,
                match kind {
                    "commit-message" => PARAMS_COMMIT_MESSAGE,
                    "atomic-commit" => PARAMS_ATOMIC,
                    "signed-commits" => PARAMS_SIGNED,
                    "dco" | "verified-commits" | "conflict-markers" | "dep-risk" | "lockfile"
                    | "docs-coupling" | "changelog" => PARAMS_NONE,
                    "large-files" => PARAMS_LARGE_FILES,
                    "forbidden-files" => PARAMS_FORBIDDEN,
                    "secrets" => PARAMS_SECRETS,
                    "portability" => PARAMS_PORTABILITY,
                    "text-hygiene" => PARAMS_TEXT_HYGIENE,
                    "format" | "typecheck" | "build" => PARAMS_QUALITY,
                    "lint" => PARAMS_LINT,
                    "tests" => PARAMS_TESTS,
                    "coverage" => PARAMS_COVERAGE,
                    "test-presence" => PARAMS_TEST_PRESENCE,
                    "dep-vulnerabilities" | "dep-licenses" | "dep-versions" => PARAMS_DEP_FULL,
                    "sarif" => PARAMS_SARIF,
                    "workflow-security" => PARAMS_WORKFLOW_SECURITY,
                    "docs-required" => PARAMS_DOCS_REQUIRED,
                    "docs-links" => PARAMS_DOCS_LINKS,
                    "docs-lint" => PARAMS_DOCS_LINT,
                    "docs-staleness" => PARAMS_DOCS_STALENESS,
                    "generated-sync" => PARAMS_GENERATED_SYNC,
                    _ => return None,
                },
            ))
        }
    }
}

fn check_gate_with(ctx: &mut Ctx, gate: &Gate, path: &str) {
    let Some((kind, params)) = params_for(gate) else {
        return; // unknown kinds are reported by check_gate_uses
    };
    let object = match &gate.with {
        serde_json::Value::Null => None,
        serde_json::Value::Object(object) => Some(object),
        _ => {
            ctx.error(
                "RW-E2019",
                "gate.with must be a table",
                &format!("{path}.with"),
                "gate-parameters",
            );
            return;
        }
    };

    if kind == "command" {
        let has_run = ["run", "run_windows", "run_linux"].iter().any(|key| {
            object
                .as_ref()
                .and_then(|object| object.get(*key))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|command| !command.trim().is_empty())
        });
        if !has_run {
            ctx.error_with_fix(
                "RW-E2019",
                format!("command gate `{}` needs a run command", gate.id),
                &format!("{path}.with.run"),
                "gate-parameters",
                "set `run`, or `run_windows` / `run_linux`",
            );
        }
    }
    if kind == "script" {
        match object
            .as_ref()
            .and_then(|object| object.get("path"))
            .and_then(serde_json::Value::as_str)
        {
            Some(script) if !script.trim().is_empty() => {
                check_glob_relative_path(
                    ctx,
                    script,
                    &format!("{path}.with.path"),
                    "gate-parameters",
                );
                if !script.replace('\\', "/").starts_with(".repowill/gates/") {
                    ctx.error_with_fix(
                        "RW-E2022",
                        "script gates must reference a file under .repowill/gates/",
                        &format!("{path}.with.path"),
                        "gate-parameters",
                        "move the script to .repowill/gates/ and use a relative path",
                    );
                }
            }
            _ => ctx.error_with_fix(
                "RW-E2019",
                format!("script gate `{}` needs a path", gate.id),
                &format!("{path}.with.path"),
                "gate-parameters",
                "set `path` to a script under .repowill/gates/",
            ),
        }
    }

    let Some(object) = object else {
        return;
    };
    for (key, value) in object {
        let param_path = format!("{path}.with.{key}");
        let Some(spec) = params.iter().find(|param| param.name == key) else {
            let suggestion = closest_name(key, params.iter().map(|param| param.name));
            ctx.push(
                "RW-E2028",
                Severity::Error,
                format!("unknown `with` parameter `{key}` for kind `{kind}`"),
                Some(param_path),
                "gate-parameters",
                suggestion.map(|name| format!("did you mean `{name}`?")),
            );
            continue;
        };
        if !param_type_matches(spec.kind, value) {
            ctx.error(
                "RW-E2019",
                format!("`with.{key}` must be {}", param_type_name(spec.kind)),
                &param_path,
                "gate-parameters",
            );
            continue;
        }
        if let (Some(allowed), Some(text)) = (spec.allowed, value.as_str())
            && !allowed.contains(&text)
        {
            ctx.error_with_fix(
                "RW-E2030",
                format!("`with.{key}` = `{text}` is not allowed"),
                &param_path,
                "gate-parameters",
                format!("allowed values: {}", allowed.join(", ")),
            );
        }
        if spec.kind == ParamType::Text
            && let Some(text) = value.as_str()
            && text.trim().is_empty()
            && !matches!(key.as_str(), "baseline" | "report" | "junit_report")
        {
            ctx.error(
                "RW-E2019",
                format!("`with.{key}` must not be empty"),
                &param_path,
                "gate-parameters",
            );
        }
        match (kind, key.as_str()) {
            ("forbidden-files", "patterns" | "allow")
            | ("atomic-commit", "ignore")
            | ("coverage", "exclude")
            | ("docs-links", "include") => {
                for (index, pattern) in string_items(value).iter().enumerate() {
                    check_glob(
                        ctx,
                        pattern,
                        &format!("{param_path}.{index}"),
                        "gate-parameters",
                    );
                }
            }
            ("sarif", "baseline") => {
                if let Some(baseline) = value.as_str()
                    && !baseline.is_empty()
                {
                    check_repo_path(ctx, baseline, &param_path, "gate-parameters");
                }
            }
            ("sarif", "sarif_path") => {
                if let Some(path) = value.as_str()
                    && path != "-"
                    && !path.is_empty()
                {
                    check_repo_path(ctx, path, &param_path, "gate-parameters");
                }
            }
            (_, "cwd") => {
                if let Some(cwd) = value.as_str() {
                    check_glob_relative_path(ctx, cwd, &param_path, "gate-parameters");
                }
            }
            _ => {}
        }
    }
}

fn string_items(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default()
}

fn param_type_matches(kind: ParamType, value: &serde_json::Value) -> bool {
    match kind {
        ParamType::Bool => value.is_boolean(),
        ParamType::Int => value.is_i64() || value.is_u64(),
        ParamType::Number => value.is_number(),
        ParamType::Text => value.is_string(),
        ParamType::TextList => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_string)),
        ParamType::IntList => value
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item.is_i64() || item.is_u64())),
        ParamType::Table => value.is_object(),
    }
}

fn param_type_name(kind: ParamType) -> &'static str {
    match kind {
        ParamType::Bool => "a boolean",
        ParamType::Int => "an integer",
        ParamType::Number => "a number",
        ParamType::Text => "a string",
        ParamType::TextList => "a list of strings",
        ParamType::IntList => "a list of integers",
        ParamType::Table => "a table",
    }
}

fn check_gate_graph(ctx: &mut Ctx, policy: &Policy) {
    let index_by_id: BTreeMap<&str, usize> = policy
        .gates
        .iter()
        .enumerate()
        .filter(|(_, gate)| !gate.id.is_empty())
        .map(|(index, gate)| (gate.id.as_str(), index))
        .collect();

    for (index, gate) in policy.gates.iter().enumerate() {
        for (dependency_index, dependency) in gate.depends_on.iter().enumerate() {
            if dependency.is_empty() {
                continue;
            }
            if dependency == &gate.id {
                ctx.error(
                    "RW-E2023",
                    format!("gate `{}` depends on itself", gate.id),
                    &format!("gates.{index}.depends_on.{dependency_index}"),
                    "gate-dependency",
                );
            } else if !index_by_id.contains_key(dependency.as_str()) {
                ctx.error_with_fix(
                    "RW-E2023",
                    format!("gate `{}` depends on unknown gate `{dependency}`", gate.id),
                    &format!("gates.{index}.depends_on.{dependency_index}"),
                    "gate-dependency",
                    "list gates before they are referenced, or remove the dependency",
                );
            }
        }
    }

    let mut state = vec![0u8; policy.gates.len()];
    let mut cycles = BTreeSet::new();
    for index in 0..policy.gates.len() {
        if state[index] == 0 {
            let mut stack = Vec::new();
            visit_gate(
                index,
                policy,
                &index_by_id,
                &mut state,
                &mut stack,
                &mut cycles,
            );
        }
    }
    for cycle in cycles {
        ctx.error(
            "RW-E2023",
            format!("gate dependency cycle: {cycle}"),
            "gates",
            "gate-dependency",
        );
    }
}

fn visit_gate(
    index: usize,
    policy: &Policy,
    index_by_id: &BTreeMap<&str, usize>,
    state: &mut [u8],
    stack: &mut Vec<usize>,
    cycles: &mut BTreeSet<String>,
) {
    state[index] = 1;
    stack.push(index);
    for dependency in &policy.gates[index].depends_on {
        if let Some(&next) = index_by_id.get(dependency.as_str()) {
            if state[next] == 0 {
                visit_gate(next, policy, index_by_id, state, stack, cycles);
            } else if state[next] == 1
                && let Some(position) = stack.iter().position(|entry| *entry == next)
            {
                let cycle: Vec<&str> = stack[position..]
                    .iter()
                    .map(|entry| policy.gates[*entry].id.as_str())
                    .collect();
                cycles.insert(cycle.join(" -> "));
            }
        }
    }
    stack.pop();
    state[index] = 2;
}

fn check_waivers(ctx: &mut Ctx, policy: &Policy) {
    check_unique_ids(
        ctx,
        &policy.waivers,
        "waivers",
        "waiver",
        "RW-E2006",
        "RW-E2007",
        "unique-id",
        false,
    );
    let gate_ids: BTreeSet<&str> = policy.gates.iter().map(|gate| gate.id.as_str()).collect();
    for (index, waiver) in policy.waivers.iter().enumerate() {
        let path = format!("waivers.{index}");
        let Some((kind, target)) = waiver.target.split_once('.') else {
            ctx.error_with_fix(
                "RW-E2024",
                format!(
                    "waiver target `{}` must be gates.<id> or guards.<name>",
                    waiver.target
                ),
                &format!("{path}.target"),
                "waiver",
                "for example target = \"gates.tests\"",
            );
            continue;
        };
        let known = match kind {
            "gates" => gate_ids.contains(target),
            "guards" => GUARD_NAMES.contains(&target),
            _ => false,
        };
        if !known {
            ctx.error(
                "RW-E2024",
                format!("waiver target `{}` does not exist", waiver.target),
                &format!("{path}.target"),
                "waiver",
            );
        }
        if waiver.reason.trim().is_empty() {
            ctx.error(
                "RW-E2034",
                "waiver needs a reason",
                &format!("{path}.reason"),
                "waiver",
            );
        }
        check_date(ctx, &waiver.expires, &format!("{path}.expires"));
        for (path_index, pattern) in waiver.paths.iter().enumerate() {
            check_glob(
                ctx,
                pattern,
                &format!("{path}.paths.{path_index}"),
                "waiver",
            );
        }
    }
}

fn check_lock(ctx: &mut Ctx, policy: &Policy) {
    let mut seen = BTreeSet::new();
    let policy_value = policy_to_value(policy).ok();
    for (index, key) in policy.lock.keys.iter().enumerate() {
        let path = format!("lock.keys.{index}");
        if key.is_empty() {
            ctx.error(
                "RW-E2033",
                "lock key must not be empty",
                &path,
                "policy-lock",
            );
            continue;
        }
        if !seen.insert(key.clone()) {
            ctx.error(
                "RW-E2033",
                format!("duplicate lock key `{key}`"),
                &path,
                "policy-lock",
            );
            continue;
        }
        let valid = key
            .split('.')
            .all(|segment| !segment.is_empty() && !segment.chars().any(char::is_whitespace));
        if !valid {
            ctx.error_with_fix(
                "RW-E2033",
                format!("`{key}` is not a valid dotted lock path"),
                &path,
                "policy-lock",
                "for example gates.tests or workflow.merge.strategy",
            );
            continue;
        }
        if let Some(value) = &policy_value
            && resolve_path(value, key).is_none()
        {
            ctx.error_with_fix(
                "RW-E2033",
                format!("lock key `{key}` does not resolve to a policy value"),
                &path,
                "policy-lock",
                "check the key path for typos; fail-closed locks reject unknown keys",
            );
        }
        if key == "lock.keys" {
            ctx.warning(
                "RW-E2033",
                "locking lock.keys has no effect: locks can only be added, never removed",
                &path,
                "policy-lock",
            );
        }
    }
}

fn check_extends(ctx: &mut Ctx, policy: &Policy) {
    let mut seen = BTreeSet::new();
    for (index, profile) in policy.extends.iter().enumerate() {
        let path = format!("extends.{index}");
        if profile.trim().is_empty() {
            ctx.error(
                "RW-E2025",
                "extends entry must not be empty",
                &path,
                "extends",
            );
            continue;
        }
        if !seen.insert(profile.clone()) {
            ctx.error(
                "RW-E2025",
                format!("duplicate extends entry `{profile}`"),
                &path,
                "extends",
            );
        }
        check_glob_relative_path(ctx, profile, &path, "extends");
        if !profile.ends_with(".toml") {
            ctx.warning(
                "RW-E2025",
                format!("extends entry `{profile}` should name a .toml profile"),
                &path,
                "extends",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Reusable checks
// ---------------------------------------------------------------------------

trait HasId {
    fn policy_id(&self) -> &str;
}

impl HasId for repowill_model::Gate {
    fn policy_id(&self) -> &str {
        &self.id
    }
}

impl HasId for repowill_model::Waiver {
    fn policy_id(&self) -> &str {
        &self.id
    }
}

impl HasId for repowill_model::ProtectedPath {
    fn policy_id(&self) -> &str {
        &self.id
    }
}

impl HasId for repowill_model::DocCoupling {
    fn policy_id(&self) -> &str {
        &self.id
    }
}

impl HasId for repowill_model::GeneratedDoc {
    fn policy_id(&self) -> &str {
        &self.id
    }
}

impl HasId for repowill_model::DepException {
    fn policy_id(&self) -> &str {
        &self.id
    }
}

impl HasId for repowill_model::IdentityEntry {
    fn policy_id(&self) -> &str {
        &self.id
    }
}

#[allow(clippy::too_many_arguments)]
fn check_unique_ids<T: HasId>(
    ctx: &mut Ctx,
    items: &[T],
    base_path: &str,
    kind: &str,
    empty_code: &str,
    duplicate_code: &str,
    rule: &str,
    kebab: bool,
) {
    let mut seen = BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let id = item.policy_id();
        let path = format!("{base_path}.{index}.id");
        if id.is_empty() {
            ctx.error(
                empty_code,
                format!("{kind} id must not be empty"),
                &path,
                rule,
            );
            continue;
        }
        let valid = if kebab {
            is_kebab_case(id)
        } else {
            is_safe_id(id)
        };
        if !valid {
            ctx.error_with_fix(
                empty_code,
                format!("`{id}` is not a valid {kind} id"),
                &path,
                rule,
                if kebab {
                    "use lowercase kebab-case, for example `unit-tests`"
                } else {
                    "use letters, digits, `-` and `_`; dots are reserved for lock paths"
                },
            );
            continue;
        }
        if !seen.insert(id.to_string()) {
            ctx.error(
                duplicate_code,
                format!("duplicate {kind} id: {id}"),
                &path,
                rule,
            );
        }
    }
}

fn is_kebab_case(text: &str) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && bytes[0].is_ascii_lowercase()
        && bytes[bytes.len() - 1].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !text.contains("--")
}

fn is_safe_id(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn is_valid_branch_type(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
}

fn looks_like_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !text.contains(char::is_whitespace)
}

fn check_enum(ctx: &mut Ctx, value: &str, path: &str, allowed: &[&str], what: &str) {
    if !allowed.contains(&value) {
        ctx.error_with_fix(
            "RW-E2030",
            format!("`{value}` is not a valid {what}"),
            path,
            "enum-value",
            format!("allowed values: {}", allowed.join(", ")),
        );
    }
}

fn check_regex(ctx: &mut Ctx, pattern: &str, path: &str, rule: &str) {
    if let Err(error) = regex::Regex::new(pattern) {
        ctx.error_with_fix(
            "RW-E2031",
            format!("invalid regular expression: {error}"),
            path,
            rule,
            "fix the regular expression syntax",
        );
    }
}

enum GlobIssue {
    Root(String),
    Compile(String),
}

fn check_glob(ctx: &mut Ctx, pattern: &str, path: &str, rule: &str) {
    match validate_glob(pattern) {
        Ok(()) => {}
        Err(GlobIssue::Root(message)) => {
            ctx.error_with_fix(
                "RW-E2022",
                message,
                path,
                rule,
                "use a repo-relative `/` glob",
            );
        }
        Err(GlobIssue::Compile(message)) => {
            ctx.error_with_fix("RW-E2005", message, path, rule, "fix the glob syntax");
        }
    }
}

fn validate_glob(pattern: &str) -> Result<(), GlobIssue> {
    if pattern.is_empty() {
        return Err(GlobIssue::Root("path glob must not be empty".to_string()));
    }
    if pattern.contains('\\') {
        return Err(GlobIssue::Root(
            "path globs must use `/` separators, not `\\`".to_string(),
        ));
    }
    if pattern.starts_with('/') || has_windows_drive(pattern) || pattern.starts_with("~/") {
        return Err(GlobIssue::Root(
            "path globs must be relative to the repository root".to_string(),
        ));
    }
    if pattern.split('/').any(|segment| segment == "..") {
        return Err(GlobIssue::Root(
            "path globs must not contain `..` path segments".to_string(),
        ));
    }
    if pattern.chars().any(|character| character.is_control()) {
        return Err(GlobIssue::Root(
            "path globs must not contain control characters".to_string(),
        ));
    }
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|_| ())
        .map_err(|error| GlobIssue::Compile(format!("invalid glob `{pattern}`: {error}")))
}

fn check_glob_relative_path(ctx: &mut Ctx, path_value: &str, path: &str, rule: &str) {
    if let Err(message) = validate_relative_path(path_value) {
        ctx.error_with_fix("RW-E2022", message, path, rule, "use a repo-relative path");
    }
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path must not be empty".to_string());
    }
    if path.contains('\\') {
        return Err("paths must use `/` separators, not `\\`".to_string());
    }
    if path.starts_with('/') || has_windows_drive(path) || path.starts_with("~/") {
        return Err("paths must be relative to the repository root".to_string());
    }
    if path.split('/').any(|segment| segment == "..") {
        return Err("paths must not contain `..` segments".to_string());
    }
    if path.chars().any(|character| character.is_control()) {
        return Err("paths must not contain control characters".to_string());
    }
    Ok(())
}

fn check_repo_path(ctx: &mut Ctx, path_value: &str, path: &str, rule: &str) {
    if let Err(message) = validate_relative_path(path_value) {
        ctx.error_with_fix("RW-E2025", message, path, rule, "use a repo-relative path");
        return;
    }
    if path_value.contains(['*', '?', '[', ']']) {
        ctx.error_with_fix(
            "RW-E2025",
            "this field must be an exact file path, not a glob",
            path,
            rule,
            "remove glob metacharacters",
        );
    }
}

fn has_windows_drive(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn check_branch_name(ctx: &mut Ctx, branch: &str, path: &str) {
    if let Err(message) = validate_branch_name(branch) {
        ctx.error_with_fix(
            "RW-E2026",
            message,
            path,
            "workflow",
            "see `git check-ref-format`",
        );
    }
}

fn validate_branch_name(branch: &str) -> Result<(), String> {
    if branch.is_empty() {
        return Err("branch name must not be empty".to_string());
    }
    if branch.starts_with('-') {
        return Err("branch name must not start with `-`".to_string());
    }
    if branch == "HEAD" || branch == "@" {
        return Err("`HEAD` and `@` are reserved".to_string());
    }
    if branch == "@{-1}" {
        return Err("reference syntax is not a branch name".to_string());
    }
    if branch.contains("..") || branch.contains("@{") {
        return Err("branch name must not contain `..` or `@{`".to_string());
    }
    if branch.ends_with('.') || branch.ends_with(".lock") || branch.ends_with('/') {
        return Err("branch name must not end with `.`, `.lock`, or `/`".to_string());
    }
    if branch
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err("branch name must not contain whitespace or control characters".to_string());
    }
    if branch.contains(['~', '^', ':', '?', '*', '[', '\\']) {
        return Err("branch name contains a character git forbids".to_string());
    }
    Ok(())
}

fn check_duration(ctx: &mut Ctx, duration: &str, path: &str, rule: &str) {
    if parse_duration(duration).is_none() {
        ctx.error_with_fix(
            "RW-E2029",
            format!("`{duration}` is not a valid duration"),
            path,
            rule,
            "use a positive duration such as \"90s\", \"10m\", or \"1h\"",
        );
    }
}

fn parse_duration(text: &str) -> Option<u128> {
    if text.is_empty() {
        return None;
    }
    let mut rest = text;
    let mut total: u128 = 0;
    let mut segments = 0;
    while !rest.is_empty() {
        let digits_end = rest
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits_end == 0 {
            return None;
        }
        let value: u128 = rest[..digits_end].parse().ok()?;
        let unit_rest = &rest[digits_end..];
        let (unit, next) = if let Some(remainder) = unit_rest.strip_prefix("ms") {
            (1, remainder)
        } else if let Some(remainder) = unit_rest.strip_prefix('s') {
            (1_000, remainder)
        } else if let Some(remainder) = unit_rest.strip_prefix('m') {
            (60_000, remainder)
        } else if let Some(remainder) = unit_rest.strip_prefix('h') {
            (3_600_000, remainder)
        } else if let Some(remainder) = unit_rest.strip_prefix('d') {
            (86_400_000, remainder)
        } else {
            return None;
        };
        total = total.checked_add(value.checked_mul(unit)?)?;
        segments += 1;
        rest = next;
    }
    (segments > 0 && total > 0).then_some(total)
}

fn check_date(ctx: &mut Ctx, date: &str, path: &str) {
    if validate_date(date).is_err() {
        ctx.error_with_fix(
            "RW-E2032",
            format!("`{date}` is not a valid date"),
            path,
            "expiry-date",
            "use YYYY-MM-DD",
        );
    }
}

fn validate_date(date: &str) -> Result<(), ()> {
    if !date.is_ascii() {
        return Err(());
    }
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(());
    }
    let year: i32 = date[..4].parse().map_err(|_| ())?;
    let month: u32 = date[5..7].parse().map_err(|_| ())?;
    let day: u32 = date[8..10].parse().map_err(|_| ())?;
    if year < 1970 || !(1..=12).contains(&month) {
        return Err(());
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(()),
    };
    if !(1..=days).contains(&day) {
        return Err(());
    }
    Ok(())
}

fn closest_name<'a>(needle: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .map(|candidate| (edit_distance(needle, candidate), candidate))
        .filter(|(distance, _)| *distance <= 2 || *distance * 3 <= needle.len().max(3))
        .min_by_key(|(distance, candidate)| (*distance, *candidate))
        .map(|(_, candidate)| candidate)
}

// ---------------------------------------------------------------------------
// Semantic version comparison (sufficient for min_repowill_version)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Semver {
    major: u64,
    minor: u64,
    patch: u64,
    pre_release: Vec<PreRelease>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreRelease {
    Numeric(u64),
    Text(String),
}

impl Semver {
    fn parse(text: &str) -> Option<Self> {
        let (without_build, _) = text
            .split_once('+')
            .map_or((text, None), |(left, right)| (left, Some(right)));
        let (core, pre_release) = without_build
            .split_once('-')
            .map_or((without_build, None), |(left, right)| (left, Some(right)));
        let mut parts = core.split('.');
        let major = parse_version_number(parts.next()?)?;
        let minor = parse_version_number(parts.next()?)?;
        let patch = parse_version_number(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        let pre_release = match pre_release {
            None => Vec::new(),
            Some(text) => {
                let mut identifiers = Vec::new();
                for identifier in text.split('.') {
                    if identifier.is_empty()
                        || !identifier
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '-')
                    {
                        return None;
                    }
                    if identifier
                        .chars()
                        .all(|character| character.is_ascii_digit())
                    {
                        identifiers.push(PreRelease::Numeric(parse_version_number(identifier)?));
                    } else {
                        identifiers.push(PreRelease::Text(identifier.to_string()));
                    }
                }
                identifiers
            }
        };
        Some(Self {
            major,
            minor,
            patch,
            pre_release,
        })
    }
}

fn parse_version_number(text: &str) -> Option<u64> {
    if text.is_empty()
        || !text.chars().all(|character| character.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return None;
    }
    text.parse().ok()
}

/// Compare two semantic version strings. Used by weakening detection.
pub(crate) fn compare_versions(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    Some(Semver::parse(left)?.cmp(&Semver::parse(right)?))
}

/// Parse a policy duration into milliseconds. Used by weakening detection.
pub(crate) fn duration_millis(text: &str) -> Option<u128> {
    parse_duration(text)
}

impl PartialOrd for Semver {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Semver {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(
                || match (self.pre_release.is_empty(), other.pre_release.is_empty()) {
                    (true, true) => std::cmp::Ordering::Equal,
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    (false, false) => compare_prerelease(&self.pre_release, &other.pre_release),
                },
            )
    }
}

fn compare_prerelease(left: &[PreRelease], right: &[PreRelease]) -> std::cmp::Ordering {
    for (a, b) in left.iter().zip(right) {
        let ordering = match (a, b) {
            (PreRelease::Numeric(a), PreRelease::Numeric(b)) => a.cmp(b),
            (PreRelease::Numeric(_), PreRelease::Text(_)) => std::cmp::Ordering::Less,
            (PreRelease::Text(_), PreRelease::Numeric(_)) => std::cmp::Ordering::Greater,
            (PreRelease::Text(a), PreRelease::Text(b)) => a.cmp(b),
        };
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use repowill_model::Policy;

    fn error_codes(policy: &Policy) -> Vec<String> {
        validate(policy)
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    #[test]
    fn default_policy_is_valid() {
        let diagnostics = validate(&Policy::default());
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn schema_version_newer_rejected() {
        let mut policy = Policy::default();
        #[allow(clippy::field_reassign_with_default)]
        {
            policy.schema_version = 2;
        }
        assert!(error_codes(&policy).contains(&"RW-E2001".to_string()));
    }

    #[test]
    fn min_version_newer_rejected() {
        let policy = Policy {
            min_repowill_version: "99.0.0".to_string(),
            ..Policy::default()
        };
        let diagnostics = validate(&policy);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "RW-E2018")
        );
        let source = "schema_version = 1\nmin_repowill_version = \"99.0.0\"\n";
        let diagnostics = validate_source(source);
        let newer = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "RW-E2018")
            .unwrap();
        assert_eq!(newer.line, Some(2));
        assert_eq!(newer.column, Some(24));
    }

    #[test]
    fn min_version_semver_ordering() {
        let version = |text: &str| Semver::parse(text).unwrap();
        assert!(version("1.0.0") < version("1.0.1"));
        assert!(version("1.0.0-alpha") < version("1.0.0"));
        assert!(version("1.0.0-alpha.2") < version("1.0.0-alpha.10"));
        assert!(Semver::parse("01.0.0").is_none());
        assert!(Semver::parse("1.0").is_none());
    }

    #[test]
    fn invalid_regex_has_location() {
        let source = r#"
schema_version = 1
[workflow.commits]
custom_pattern = "("
"#;
        let diagnostics = validate_source(source);
        let regex = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "RW-E2031")
            .unwrap();
        assert_eq!(regex.line, Some(4));
        assert_eq!(regex.column, Some(18));
    }

    #[test]
    fn invalid_glob_root_is_rejected() {
        let mut policy = Policy::default();
        policy.gates.push(test_gate("x", vec!["pre-commit"]));
        policy.gates[0].paths.include = vec!["C:/absolute".into()];
        assert!(error_codes(&policy).contains(&"RW-E2022".to_string()));
        policy.gates[0].paths.include = vec!["src\\win".into()];
        assert!(error_codes(&policy).contains(&"RW-E2022".to_string()));
        policy.gates[0].paths.include = vec!["../escape".into()];
        assert!(error_codes(&policy).contains(&"RW-E2022".to_string()));
        policy.gates[0].paths.include = vec!["[".into()];
        assert!(error_codes(&policy).contains(&"RW-E2005".to_string()));
    }

    #[test]
    fn duplicate_ids_report_only_duplicates() {
        let mut policy = Policy::default();
        policy.gates.push(test_gate("x", vec!["pre-commit"]));
        policy.gates.push(test_gate("x", vec!["pre-commit"]));
        let diagnostics = validate(&policy);
        let duplicates: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "RW-E2003")
            .collect();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].line, None);
    }

    #[test]
    fn empty_gate_id_reports_once() {
        let mut policy = Policy::default();
        policy.gates.push(test_gate("", vec!["pre-commit"]));
        let diagnostics = validate(&policy);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "RW-E2002")
                .count(),
            1
        );
    }

    #[test]
    fn gate_kind_and_stage_validation() {
        let mut policy = Policy::default();
        let mut gate = test_gate("x", vec!["nope"]);
        gate.uses = "builtin:unknown".into();
        policy.gates.push(gate);
        let codes = error_codes(&policy);
        assert!(codes.contains(&"RW-E2014".to_string()));
        assert!(codes.contains(&"RW-E2017".to_string()));
    }

    #[test]
    fn gate_with_unknown_parameter_suggests() {
        let mut policy = Policy::default();
        let mut gate = test_gate("x", vec!["pre-commit"]);
        gate.uses = "builtin:large-files".into();
        gate.with = serde_json::json!({ "max_kbb": 10 });
        policy.gates.push(gate);
        let diagnostics = validate(&policy);
        let unknown = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "RW-E2028")
            .unwrap();
        assert_eq!(unknown.fix.as_deref(), Some("did you mean `max_kb`?"));
    }

    #[test]
    fn command_gate_requires_run() {
        let mut policy = Policy::default();
        let mut gate = test_gate("x", vec!["pre-commit"]);
        gate.uses = "command".into();
        policy.gates.push(gate);
        assert!(error_codes(&policy).contains(&"RW-E2019".to_string()));
    }

    #[test]
    fn script_gate_path_must_be_under_repowill() {
        let mut policy = Policy::default();
        let mut gate = test_gate("x", vec!["pre-commit"]);
        gate.uses = "script".into();
        gate.with = serde_json::json!({ "path": "scripts/lint.sh" });
        policy.gates.push(gate);
        assert!(error_codes(&policy).contains(&"RW-E2022".to_string()));

        policy.gates[0].with = serde_json::json!({ "path": ".repowill/gates/lint.sh" });
        assert!(!error_codes(&policy).contains(&"RW-E2022".to_string()));
    }

    #[test]
    fn dependency_cycle_detected() {
        let mut policy = Policy::default();
        let mut first = test_gate("a", vec!["pre-commit"]);
        first.depends_on = vec!["b".into()];
        let mut second = test_gate("b", vec!["pre-commit"]);
        second.depends_on = vec!["a".into()];
        policy.gates = vec![first, second];
        assert!(error_codes(&policy).contains(&"RW-E2023".to_string()));
    }

    #[test]
    fn waiver_target_must_exist() {
        let mut policy = Policy::default();
        policy.waivers.push(repowill_model::Waiver {
            id: "w".into(),
            target: "gates.missing".into(),
            reason: "because".into(),
            paths: vec![],
            expires: "2026-12-31".into(),
            request: String::new(),
            remove: false,
        });
        assert!(error_codes(&policy).contains(&"RW-E2024".to_string()));
    }

    #[test]
    fn lock_key_must_resolve() {
        let mut policy = Policy::default();
        policy.lock.keys = vec!["gates.missing".into()];
        assert!(error_codes(&policy).contains(&"RW-E2033".to_string()));
        policy.lock.keys = vec!["guards.main_protection".into()];
        assert!(!error_codes(&policy).contains(&"RW-E2033".to_string()));
    }

    #[test]
    fn duration_parser() {
        assert!(parse_duration("90s").is_some());
        assert!(parse_duration("10m").is_some());
        assert!(parse_duration("1h30m").is_some());
        assert!(parse_duration("0s").is_none());
        assert!(parse_duration("10").is_none());
        assert!(parse_duration("1w").is_none());
    }

    #[test]
    fn date_parser() {
        assert!(validate_date("2026-12-31").is_ok());
        assert!(validate_date("2024-02-29").is_ok());
        assert!(validate_date("2023-02-29").is_err());
        assert!(validate_date("2026-13-01").is_err());
        assert!(validate_date("26-01-01").is_err());
    }

    #[test]
    fn parse_error_has_line_and_column() {
        let diagnostics = validate_source("schema_version = \n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "RW-E2000");
        assert!(diagnostics[0].line.is_some());
        assert!(diagnostics[0].column.is_some());
    }

    #[test]
    fn unknown_field_suggests_known_key() {
        let diagnostics = validate_source("schema_version = 1\nmin_repowill_versio = \"1.0.0\"\n");
        assert_eq!(diagnostics[0].code, "RW-E2000");
        assert_eq!(
            diagnostics[0].fix.as_deref(),
            Some("did you mean `min_repowill_version`?")
        );
    }

    fn test_gate(id: &str, stages: Vec<&str>) -> Gate {
        Gate {
            id: id.into(),
            name: format!("Gate {id}"),
            description: String::new(),
            uses: "builtin:dco".into(),
            with: serde_json::Value::Null,
            stages: stages.into_iter().map(str::to_string).collect(),
            mode: Mode::Block,
            stage_modes: BTreeMap::new(),
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
        }
    }
}
