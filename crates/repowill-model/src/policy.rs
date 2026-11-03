// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema_version: u32,
    #[serde(default = "default_min_repowill_version")]
    pub min_repowill_version: String,
    #[serde(default)]
    pub extends: Vec<String>,

    #[serde(default)]
    pub project: Project,
    #[serde(default)]
    pub identity: Identity,
    #[serde(default)]
    pub workflow: Workflow,
    #[serde(default)]
    pub guards: Guards,
    #[serde(default)]
    pub protection: Protection,
    #[serde(default)]
    pub dependencies: Dependencies,
    #[serde(default)]
    pub docs: Docs,
    #[serde(default)]
    pub agents: Agents,
    #[serde(default)]
    pub remote: Remote,
    #[serde(default)]
    pub engine: Engine,

    #[serde(default, rename = "gates")]
    pub gates: Vec<Gate>,
    #[serde(default, rename = "waivers")]
    pub waivers: Vec<Waiver>,
    #[serde(default)]
    pub lock: std::collections::BTreeMap<String, bool>,
}

fn default_min_repowill_version() -> String {
    "1.0.0".to_string()
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            min_repowill_version: default_min_repowill_version(),
            extends: vec![],
            project: Project::default(),
            identity: Identity::default(),
            workflow: Workflow::default(),
            guards: Guards::default(),
            protection: Protection::default(),
            dependencies: Dependencies::default(),
            docs: Docs::default(),
            agents: Agents::default(),
            remote: Remote::default(),
            engine: Engine::default(),
            gates: vec![],
            waivers: vec![],
            lock: std::collections::BTreeMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// 3.2 project
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub stacks: Vec<String>,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default = "default_profile")]
    pub profile: String,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            license: String::new(),
            stacks: vec![],
            preset: default_preset(),
            profile: default_profile(),
        }
    }
}

fn default_preset() -> String {
    "recommended".to_string()
}
fn default_profile() -> String {
    "default".to_string()
}

// ---------------------------------------------------------------------------
// 3.3 identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default = "default_mode_block")]
    pub enforce: Mode,
    #[serde(default = "default_true")]
    pub check_committer: bool,
    #[serde(default)]
    pub allowed: Vec<IdentityEntry>,
    #[serde(default)]
    pub signing: SigningMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityEntry {
    pub id: String,
    pub name: String,
    pub email: String,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            name: String::new(),
            email: String::new(),
            enforce: Mode::Block,
            check_committer: true,
            allowed: vec![],
            signing: SigningMode::Off,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Block,
    Warn,
    Off,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum SigningMode {
    #[default]
    Off,
    Ssh,
    Gpg,
    Any,
}

// ---------------------------------------------------------------------------
// 3.4 workflow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    #[serde(default = "default_workflow_model")]
    pub model: WorkflowModel,
    #[serde(default = "default_main_branch")]
    pub main_branch: String,
    #[serde(default = "default_protected_branches")]
    pub protected_branches: Vec<String>,
    #[serde(default)]
    pub integration_branch: String,
    #[serde(default = "default_allowed_bases")]
    pub allowed_bases: Vec<String>,
    #[serde(default)]
    pub bases_by_type: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub branch_from_tip: bool,
    #[serde(default)]
    pub allow_stacked: bool,
    #[serde(default)]
    pub stacked_patterns: Vec<String>,
    #[serde(default = "default_stale_branch_days")]
    pub stale_branch_days: i64,
    #[serde(default = "default_max_commits_ahead")]
    pub max_commits_ahead: i64,

    #[serde(default)]
    pub branch_naming: BranchNaming,
    #[serde(default)]
    pub commits: Commits,
    #[serde(default)]
    pub merge: Merge,
    #[serde(default)]
    pub push: Push,
    #[serde(default)]
    pub tags: Tags,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowModel {
    #[default]
    StrictMainline,
    GithubFlow,
    Gitflow,
    Custom,
}

impl Default for Workflow {
    fn default() -> Self {
        Self {
            model: WorkflowModel::StrictMainline,
            main_branch: default_main_branch(),
            protected_branches: default_protected_branches(),
            integration_branch: String::new(),
            allowed_bases: default_allowed_bases(),
            bases_by_type: std::collections::BTreeMap::new(),
            branch_from_tip: false,
            allow_stacked: false,
            stacked_patterns: vec![],
            stale_branch_days: default_stale_branch_days(),
            max_commits_ahead: default_max_commits_ahead(),
            branch_naming: BranchNaming::default(),
            commits: Commits::default(),
            merge: Merge::default(),
            push: Push::default(),
            tags: Tags::default(),
        }
    }
}

fn default_workflow_model() -> WorkflowModel {
    WorkflowModel::StrictMainline
}
fn default_main_branch() -> String {
    "main".to_string()
}
fn default_protected_branches() -> Vec<String> {
    vec!["main".to_string()]
}
fn default_allowed_bases() -> Vec<String> {
    vec!["main".to_string()]
}
fn default_allowed_registries() -> Vec<String> {
    vec!["default".to_string()]
}
fn default_stale_branch_days() -> i64 {
    14
}
fn default_max_commits_ahead() -> i64 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BranchNaming {
    #[serde(default = "default_mode_block")]
    pub mode: Mode,
    #[serde(default = "default_branch_types")]
    pub types: Vec<String>,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub exceptions: Vec<String>,
}

impl Default for BranchNaming {
    fn default() -> Self {
        Self {
            mode: Mode::Block,
            types: default_branch_types(),
            pattern: String::new(),
            exceptions: vec![],
        }
    }
}

fn default_branch_types() -> Vec<String> {
    vec![
        "feat".into(),
        "fix".into(),
        "docs".into(),
        "refactor".into(),
        "perf".into(),
        "test".into(),
        "build".into(),
        "ci".into(),
        "chore".into(),
        "hotfix".into(),
        "release".into(),
        "revert".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Commits {
    #[serde(default = "default_convention")]
    pub convention: Convention,
    #[serde(default = "default_commit_types")]
    pub types: Vec<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub require_scope: bool,
    #[serde(default = "default_subject_max")]
    pub subject_max: i64,
    #[serde(default = "default_true")]
    pub subject_lowercase_start: bool,
    #[serde(default = "default_true")]
    pub forbid_trailing_period: bool,
    #[serde(default = "default_body_max_line")]
    pub body_max_line: i64,
    #[serde(default)]
    pub require_body_for: Vec<String>,
    #[serde(default)]
    pub require_issue_ref: bool,
    #[serde(default = "default_issue_pattern")]
    pub issue_pattern: String,
    #[serde(default)]
    pub required_trailers: Vec<String>,
    #[serde(default)]
    pub custom_pattern: String,
    #[serde(default = "default_atomic")]
    pub atomic: AtomicLevel,
    #[serde(default = "default_atomic_max_files")]
    pub atomic_max_files: i64,
    #[serde(default = "default_atomic_max_lines")]
    pub atomic_max_lines: i64,
    #[serde(default)]
    pub atomic_single_scope: bool,
    #[serde(default)]
    pub scope_map: std::collections::BTreeMap<String, String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Convention {
    #[default]
    Conventional,
    Free,
    Custom,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AtomicLevel {
    Off,
    #[default]
    Standard,
    Strict,
}

impl Default for Commits {
    fn default() -> Self {
        Self {
            convention: Convention::Conventional,
            types: default_commit_types(),
            scopes: vec![],
            require_scope: false,
            subject_max: default_subject_max(),
            subject_lowercase_start: true,
            forbid_trailing_period: true,
            body_max_line: default_body_max_line(),
            require_body_for: vec![],
            require_issue_ref: false,
            issue_pattern: default_issue_pattern(),
            required_trailers: vec![],
            custom_pattern: String::new(),
            atomic: AtomicLevel::Standard,
            atomic_max_files: default_atomic_max_files(),
            atomic_max_lines: default_atomic_max_lines(),
            atomic_single_scope: false,
            scope_map: std::collections::BTreeMap::new(),
        }
    }
}

fn default_convention() -> Convention {
    Convention::Conventional
}
fn default_commit_types() -> Vec<String> {
    vec![
        "feat".into(),
        "fix".into(),
        "docs".into(),
        "style".into(),
        "refactor".into(),
        "perf".into(),
        "test".into(),
        "build".into(),
        "ci".into(),
        "chore".into(),
        "revert".into(),
    ]
}
fn default_subject_max() -> i64 {
    72
}
fn default_body_max_line() -> i64 {
    100
}
fn default_issue_pattern() -> String {
    "#\\d+".to_string()
}
fn default_atomic() -> AtomicLevel {
    AtomicLevel::Standard
}
fn default_atomic_max_files() -> i64 {
    20
}
fn default_atomic_max_lines() -> i64 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Merge {
    #[serde(default = "default_merge_strategy")]
    pub strategy: MergeStrategy,
    #[serde(default = "default_merge_location")]
    pub location: MergeLocation,
    #[serde(default = "default_true")]
    pub require_approval: bool,
    #[serde(default = "default_approval_expiry_hours")]
    pub approval_expiry_hours: i64,
    #[serde(default = "default_up_to_date")]
    pub up_to_date: UpToDate,
    #[serde(default = "default_true")]
    pub run_gates_on_result: bool,
    #[serde(default = "default_message_template")]
    pub message_template: String,
    #[serde(default = "default_true")]
    pub trailers: bool,
    #[serde(default = "default_after_merge")]
    pub after_merge: AfterMerge,
    #[serde(default = "default_approval_source")]
    pub approval_source: ApprovalSource,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MergeStrategy {
    #[default]
    NoFf,
    Squash,
    FfOnly,
    RebaseMerge,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MergeLocation {
    #[default]
    Auto,
    Local,
    GithubPr,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum UpToDate {
    #[default]
    TrialMerge,
    ContainsMain,
    Off,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AfterMerge {
    #[default]
    Keep,
    ArchiveAndRequestDelete,
    RequestDelete,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalSource {
    #[default]
    Local,
    GithubReview,
    Either,
}

impl Default for Merge {
    fn default() -> Self {
        Self {
            strategy: MergeStrategy::NoFf,
            location: MergeLocation::Auto,
            require_approval: true,
            approval_expiry_hours: default_approval_expiry_hours(),
            up_to_date: UpToDate::TrialMerge,
            run_gates_on_result: true,
            message_template: default_message_template(),
            trailers: true,
            after_merge: AfterMerge::Keep,
            approval_source: ApprovalSource::Local,
        }
    }
}

fn default_merge_strategy() -> MergeStrategy {
    MergeStrategy::NoFf
}
fn default_merge_location() -> MergeLocation {
    MergeLocation::Auto
}
fn default_approval_expiry_hours() -> i64 {
    24
}
fn default_up_to_date() -> UpToDate {
    UpToDate::TrialMerge
}
fn default_message_template() -> String {
    "Merge branch '{branch}' into {target}".to_string()
}
fn default_after_merge() -> AfterMerge {
    AfterMerge::Keep
}
fn default_approval_source() -> ApprovalSource {
    ApprovalSource::Local
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Push {
    #[serde(default = "default_allow_force")]
    pub allow_force: AllowForce,
    #[serde(default = "default_protected_direct")]
    pub protected_direct: ProtectedDirect,
    #[serde(default = "default_verified_commits")]
    pub verified_commits: Mode,
    #[serde(default = "default_remote_deletions")]
    pub remote_deletions: RemoteDeletions,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AllowForce {
    #[default]
    Never,
    OwnUnmergedWithLease,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectedDirect {
    #[default]
    AfterLocalMerge,
    Never,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteDeletions {
    #[default]
    Approval,
    Never,
}

impl Default for Push {
    fn default() -> Self {
        Self {
            allow_force: AllowForce::Never,
            protected_direct: ProtectedDirect::AfterLocalMerge,
            verified_commits: Mode::Warn,
            remote_deletions: RemoteDeletions::Approval,
        }
    }
}

fn default_allow_force() -> AllowForce {
    AllowForce::Never
}
fn default_protected_direct() -> ProtectedDirect {
    ProtectedDirect::AfterLocalMerge
}
fn default_verified_commits() -> Mode {
    Mode::Warn
}
fn default_remote_deletions() -> RemoteDeletions {
    RemoteDeletions::Approval
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tags {
    #[serde(default = "default_tag_pattern")]
    pub pattern: String,
    #[serde(default = "default_true")]
    pub annotated: bool,
    #[serde(default = "default_only_on")]
    pub only_on: Vec<String>,
    #[serde(default = "default_true")]
    pub protected: bool,
}

impl Default for Tags {
    fn default() -> Self {
        Self {
            pattern: default_tag_pattern(),
            annotated: true,
            only_on: default_only_on(),
            protected: true,
        }
    }
}

fn default_tag_pattern() -> String {
    "^v(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)$".to_string()
}
fn default_only_on() -> Vec<String> {
    vec!["main".to_string()]
}

// ---------------------------------------------------------------------------
// 3.5 guards
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Guards {
    #[serde(default = "default_mode_block")]
    pub main_protection: Mode,
    #[serde(default = "default_mode_block")]
    pub branch_origin: Mode,
    #[serde(default = "default_mode_block")]
    pub branch_naming: Mode,
    #[serde(default = "default_mode_block")]
    pub history_protection: Mode,
    #[serde(default = "default_mode_block")]
    pub delete_guard: Mode,
    #[serde(default = "default_mode_block")]
    pub protected_paths: Mode,
    #[serde(default = "default_mode_block")]
    pub policy_guard: Mode,
    #[serde(default = "default_mode_block")]
    pub identity: Mode,
    #[serde(default = "default_mode_block")]
    pub merge_guard: Mode,
    #[serde(default = "default_mode_block")]
    pub push_guard: Mode,
    #[serde(default = "default_mode_block")]
    pub agent_guard: Mode,
}

impl Default for Guards {
    fn default() -> Self {
        Self {
            main_protection: Mode::Block,
            branch_origin: Mode::Block,
            branch_naming: Mode::Block,
            history_protection: Mode::Block,
            delete_guard: Mode::Block,
            protected_paths: Mode::Block,
            policy_guard: Mode::Block,
            identity: Mode::Block,
            merge_guard: Mode::Block,
            push_guard: Mode::Block,
            agent_guard: Mode::Block,
        }
    }
}

// ---------------------------------------------------------------------------
// 3.6 protection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Protection {
    #[serde(default)]
    pub protected_paths: Vec<ProtectedPath>,
    #[serde(default)]
    pub delete: DeleteProtection,
    #[serde(default)]
    pub policy: PolicyProtection,
    #[serde(default)]
    pub snapshots: Snapshots,
    #[serde(default)]
    pub integrity: Integrity,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProtectedPath {
    pub id: String,
    pub pattern: String,
    #[serde(default = "default_mode_approval")]
    pub mode: ProtectedMode,
    #[serde(default)]
    pub reason: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectedMode {
    #[default]
    Approval,
    Warn,
    Immutable,
}

fn default_mode_approval() -> ProtectedMode {
    ProtectedMode::Approval
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteProtection {
    #[serde(default = "default_mode_approval_str")]
    pub files: String,
    #[serde(default = "default_mode_approval_str")]
    pub branches: String,
    #[serde(default = "default_mode_approval_str")]
    pub tags: String,
    #[serde(default = "default_allow_patterns")]
    pub allow_patterns: Vec<String>,
    #[serde(default = "default_true")]
    pub recoverable: bool,
    #[serde(default = "default_true")]
    pub archive_branches: bool,
}

impl Default for DeleteProtection {
    fn default() -> Self {
        Self {
            files: "approval".to_string(),
            branches: "approval".to_string(),
            tags: "approval".to_string(),
            allow_patterns: default_allow_patterns(),
            recoverable: true,
            archive_branches: true,
        }
    }
}

fn default_mode_approval_str() -> String {
    "approval".to_string()
}
fn default_allow_patterns() -> Vec<String> {
    vec![
        "**/*.log".into(),
        "**/tmp/**".into(),
        "dist/**".into(),
        "build/**".into(),
        "coverage/**".into(),
        "target/**".into(),
        "node_modules/**".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyProtection {
    #[serde(default = "default_weakening")]
    pub weakening: WeakeningMode,
    #[serde(default = "default_true")]
    pub require_trust_for_commands: bool,
    #[serde(default)]
    pub allow_local_weakening: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum WeakeningMode {
    #[default]
    Approval,
    Warn,
}

impl Default for PolicyProtection {
    fn default() -> Self {
        Self {
            weakening: WeakeningMode::Approval,
            require_trust_for_commands: true,
            allow_local_weakening: false,
        }
    }
}

fn default_weakening() -> WeakeningMode {
    WeakeningMode::Approval
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Snapshots {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub on_agent_session: bool,
    #[serde(default = "default_true")]
    pub before_risky_ops: bool,
    #[serde(default = "default_interval_minutes")]
    pub interval_minutes: i64,
    #[serde(default = "default_true")]
    pub include_untracked: bool,
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: i64,
    #[serde(default = "default_retention_days")]
    pub retention_days: i64,
    #[serde(default = "default_max_count")]
    pub max_count: i64,
}

impl Default for Snapshots {
    fn default() -> Self {
        Self {
            enabled: true,
            on_agent_session: true,
            before_risky_ops: true,
            interval_minutes: default_interval_minutes(),
            include_untracked: true,
            max_file_size_mb: default_max_file_size_mb(),
            retention_days: default_retention_days(),
            max_count: default_max_count(),
        }
    }
}

fn default_interval_minutes() -> i64 {
    10
}
fn default_max_file_size_mb() -> i64 {
    50
}
fn default_retention_days() -> i64 {
    14
}
fn default_max_count() -> i64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Integrity {
    #[serde(default = "default_check_on")]
    pub check_on: CheckOn,
    #[serde(default = "default_on_violation")]
    pub on_violation: OnViolation,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum CheckOn {
    #[default]
    EveryHook,
    Daily,
    Manual,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum OnViolation {
    #[default]
    Block,
    Warn,
}

impl Default for Integrity {
    fn default() -> Self {
        Self {
            check_on: CheckOn::EveryHook,
            on_violation: OnViolation::Block,
        }
    }
}

fn default_check_on() -> CheckOn {
    CheckOn::EveryHook
}
fn default_on_violation() -> OnViolation {
    OnViolation::Block
}

// ---------------------------------------------------------------------------
// 3.7 dependencies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Dependencies {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_ecosystems")]
    pub ecosystems: Vec<String>,
    #[serde(default = "default_scope")]
    pub scope: String,

    #[serde(default)]
    pub vulnerabilities: VulnConfig,
    #[serde(default)]
    pub licenses: LicenseConfig,
    #[serde(default)]
    pub versions: VersionConfig,
    #[serde(default)]
    pub sources: SourceConfig,
    #[serde(default)]
    pub risk: RiskConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub exceptions: Vec<DepException>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VulnConfig {
    #[serde(default = "default_block_at")]
    pub block_at: String,
    #[serde(default = "default_warn_at")]
    pub warn_at: String,
    #[serde(default = "default_unknown_severity")]
    pub unknown_severity: String,
    #[serde(default = "default_true")]
    pub include_dev: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LicenseConfig {
    #[serde(default = "default_true")]
    pub open_source_only: bool,
    #[serde(default = "default_allowed_licenses")]
    pub allowed: Vec<String>,
    #[serde(default = "default_content_licenses")]
    pub content_licenses: Vec<String>,
    #[serde(default)]
    pub denied: Vec<String>,
    #[serde(default = "default_unknown_block")]
    pub unknown: String,
    #[serde(default = "default_true")]
    pub include_dev: bool,
    #[serde(default = "default_true")]
    pub compat_hint: bool,
}

impl Default for LicenseConfig {
    fn default() -> Self {
        Self {
            open_source_only: true,
            allowed: default_allowed_licenses(),
            content_licenses: default_content_licenses(),
            denied: vec![],
            unknown: "block".to_string(),
            include_dev: true,
            compat_hint: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionConfig {
    #[serde(default = "default_true")]
    pub require_latest_stable: bool,
    #[serde(default = "default_applies_to")]
    pub applies_to: String,
    #[serde(default = "default_tolerance")]
    pub tolerance: String,
    #[serde(default = "default_min_release_age_days")]
    pub min_release_age_days: i64,
    #[serde(default)]
    pub allow_prerelease: bool,
    #[serde(default = "default_outdated_existing")]
    pub outdated_existing: String,
    #[serde(default = "default_outdated_grace_days")]
    pub outdated_grace_days: i64,
    #[serde(default = "default_true")]
    pub block_deprecated: bool,
    #[serde(default = "default_true")]
    pub block_yanked: bool,
}

impl Default for VersionConfig {
    fn default() -> Self {
        Self {
            require_latest_stable: true,
            applies_to: "direct".to_string(),
            tolerance: "exact".to_string(),
            min_release_age_days: 7,
            allow_prerelease: false,
            outdated_existing: "warn".to_string(),
            outdated_grace_days: 30,
            block_deprecated: true,
            block_yanked: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    #[serde(default = "default_true")]
    pub require_lockfile: bool,
    #[serde(default)]
    pub allow_git: bool,
    #[serde(default)]
    pub allow_url: bool,
    #[serde(default = "default_true")]
    pub allow_path: bool,
    #[serde(default = "default_allowed_registries")]
    pub allowed_registries: Vec<String>,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            require_lockfile: true,
            allow_git: false,
            allow_url: false,
            allow_path: true,
            allowed_registries: vec!["default".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RiskConfig {
    #[serde(default = "default_install_scripts")]
    pub install_scripts: String,
    #[serde(default = "default_unmaintained_days")]
    pub unmaintained_days: i64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            install_scripts: "warn".to_string(),
            unmaintained_days: 730,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    #[serde(default = "default_network_mode")]
    pub mode: String,
    #[serde(default = "default_cache_ttl_hours")]
    pub cache_ttl_hours: i64,
    #[serde(default = "default_on_unavailable")]
    pub on_unavailable: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: "online".to_string(),
            cache_ttl_hours: 24,
            on_unavailable: "warn".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DepException {
    pub id: String,
    pub package: String,
    pub versions: String,
    pub allow: Vec<String>,
    pub reason: String,
    pub expires: String,
    pub request: String,
}

impl Default for Dependencies {
    fn default() -> Self {
        Self {
            enabled: true,
            ecosystems: vec!["auto".to_string()],
            scope: "new-and-changed".to_string(),
            vulnerabilities: VulnConfig::default(),
            licenses: LicenseConfig::default(),
            versions: VersionConfig::default(),
            sources: SourceConfig::default(),
            risk: RiskConfig::default(),
            network: NetworkConfig::default(),
            exceptions: vec![],
        }
    }
}

fn default_ecosystems() -> Vec<String> {
    vec!["auto".to_string()]
}
fn default_scope() -> String {
    "new-and-changed".to_string()
}
fn default_block_at() -> String {
    "high".to_string()
}
fn default_warn_at() -> String {
    "medium".to_string()
}
fn default_unknown_severity() -> String {
    "warn".to_string()
}
fn default_unknown_block() -> String {
    "block".to_string()
}
fn default_allowed_licenses() -> Vec<String> {
    vec![
        "MIT".into(),
        "Apache-2.0".into(),
        "BSD-2-Clause".into(),
        "BSD-3-Clause".into(),
        "ISC".into(),
        "0BSD".into(),
        "Zlib".into(),
        "Unlicense".into(),
        "CC0-1.0".into(),
        "MPL-2.0".into(),
        "Unicode-3.0".into(),
        "BSL-1.0".into(),
    ]
}
fn default_content_licenses() -> Vec<String> {
    vec!["CC-BY-4.0".into(), "OFL-1.1".into()]
}
fn default_applies_to() -> String {
    "direct".to_string()
}
fn default_tolerance() -> String {
    "exact".to_string()
}
fn default_min_release_age_days() -> i64 {
    7
}
fn default_outdated_existing() -> String {
    "warn".to_string()
}
fn default_outdated_grace_days() -> i64 {
    30
}
fn default_install_scripts() -> String {
    "warn".to_string()
}
fn default_unmaintained_days() -> i64 {
    730
}
fn default_network_mode() -> String {
    "online".to_string()
}
fn default_cache_ttl_hours() -> i64 {
    24
}
fn default_on_unavailable() -> String {
    "warn".to_string()
}

impl Default for VulnConfig {
    fn default() -> Self {
        Self {
            block_at: "high".to_string(),
            warn_at: "medium".to_string(),
            unknown_severity: "warn".to_string(),
            include_dev: true,
        }
    }
}

// ---------------------------------------------------------------------------
// 3.8 docs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Docs {
    #[serde(default = "default_required_docs")]
    pub required: Vec<String>,
    #[serde(default = "default_staleness_days")]
    pub staleness_days: i64,
    #[serde(default = "default_staleness_key")]
    pub staleness_key: String,
    #[serde(default = "default_not_needed_trailer")]
    pub not_needed_trailer: String,
    #[serde(default)]
    pub coupling: Vec<DocCoupling>,
    #[serde(default)]
    pub changelog: Changelog,
    #[serde(default)]
    pub generated: Vec<GeneratedDoc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocCoupling {
    pub id: String,
    #[serde(default)]
    pub when_changed: Vec<String>,
    #[serde(default)]
    pub require_changed: Vec<String>,
    #[serde(default = "default_mode_warn")]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Changelog {
    #[serde(default = "default_changelog_file")]
    pub file: String,
    #[serde(default = "default_changelog_format")]
    pub format: String,
    #[serde(default)]
    pub require_for: Vec<String>,
    #[serde(default = "default_mode_warn")]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeneratedDoc {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub outputs: Vec<String>,
}

impl Default for Docs {
    fn default() -> Self {
        Self {
            required: default_required_docs(),
            staleness_days: 180,
            staleness_key: "last_reviewed".to_string(),
            not_needed_trailer: "Docs-Not-Needed".to_string(),
            coupling: vec![],
            changelog: Changelog::default(),
            generated: vec![],
        }
    }
}

fn default_required_docs() -> Vec<String> {
    vec!["README.md".into(), "LICENSE".into(), "CHANGELOG.md".into()]
}
fn default_staleness_days() -> i64 {
    180
}
fn default_staleness_key() -> String {
    "last_reviewed".to_string()
}
fn default_not_needed_trailer() -> String {
    "Docs-Not-Needed".to_string()
}
fn default_mode_warn() -> String {
    "warn".to_string()
}
fn default_changelog_file() -> String {
    "CHANGELOG.md".to_string()
}
fn default_changelog_format() -> String {
    "keep-a-changelog".to_string()
}

// ---------------------------------------------------------------------------
// 3.9 agents
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Agents {
    #[serde(default = "default_agents_enabled")]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub guard: AgentGuard,
    #[serde(default)]
    pub stop_check: StopCheck,
    #[serde(default)]
    pub parallel: Parallel,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentGuard {
    #[serde(default = "default_true")]
    pub block_delete: bool,
    #[serde(default = "default_true")]
    pub block_destructive_git: bool,
    #[serde(default = "default_true")]
    pub block_hook_bypass: bool,
    #[serde(default = "default_true")]
    pub block_privilege: bool,
    #[serde(default = "default_true")]
    pub block_remote_exec: bool,
    #[serde(default = "default_true")]
    pub protect_repowill: bool,
    #[serde(default = "default_deny_read")]
    pub deny_read: Vec<String>,
    #[serde(default = "default_allow_read")]
    pub allow_read: Vec<String>,
    #[serde(default = "default_outside_workspace")]
    pub outside_workspace: String,
    #[serde(default = "default_true")]
    pub package_guard: bool,
    #[serde(default = "default_unknown_command")]
    pub unknown_command: String,
}

impl Default for AgentGuard {
    fn default() -> Self {
        Self {
            block_delete: true,
            block_destructive_git: true,
            block_hook_bypass: true,
            block_privilege: true,
            block_remote_exec: true,
            protect_repowill: true,
            deny_read: default_deny_read(),
            allow_read: default_allow_read(),
            outside_workspace: "deny".to_string(),
            package_guard: true,
            unknown_command: "allow".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StopCheck {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_gate_tags")]
    pub gate_tags: Vec<String>,
    #[serde(default = "default_max_feedback_lines")]
    pub max_feedback_lines: i64,
}

impl Default for StopCheck {
    fn default() -> Self {
        Self {
            enabled: true,
            gate_tags: vec!["fast".to_string()],
            max_feedback_lines: 40,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parallel {
    #[serde(default = "default_parallel_mode")]
    pub mode: String,
}

impl Default for Parallel {
    fn default() -> Self {
        Self {
            mode: "worktrees".to_string(),
        }
    }
}

impl Default for Agents {
    fn default() -> Self {
        Self {
            enabled: default_agents_enabled(),
            guard: AgentGuard::default(),
            stop_check: StopCheck::default(),
            parallel: Parallel::default(),
        }
    }
}

fn default_agents_enabled() -> Vec<String> {
    vec!["opencode".into(), "claude-code".into(), "generic".into()]
}
fn default_deny_read() -> Vec<String> {
    vec![
        "**/.env".into(),
        "**/.env.*".into(),
        "**/*.pem".into(),
        "**/*.key".into(),
        "**/id_rsa*".into(),
        "**/id_ed25519*".into(),
    ]
}
fn default_allow_read() -> Vec<String> {
    vec!["**/.env.example".into()]
}
fn default_outside_workspace() -> String {
    "deny".to_string()
}
fn default_unknown_command() -> String {
    "allow".to_string()
}
fn default_gate_tags() -> Vec<String> {
    vec!["fast".to_string()]
}
fn default_max_feedback_lines() -> i64 {
    40
}
fn default_parallel_mode() -> String {
    "worktrees".to_string()
}

// ---------------------------------------------------------------------------
// 3.10 remote
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    #[serde(default = "default_remote_provider")]
    pub provider: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default = "default_remote_name")]
    pub remote_name: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub github: RemoteGithub,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteGithub {
    #[serde(default = "default_rulesets")]
    pub rulesets: String,
    #[serde(default = "default_required_check")]
    pub required_check: String,
    #[serde(default = "default_true")]
    pub ci_workflow: bool,
    #[serde(default = "default_ci_runs_on")]
    pub ci_runs_on: String,
    #[serde(default)]
    pub push_archive_tags: bool,
}

impl Default for Remote {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            owner: String::new(),
            repo: String::new(),
            remote_name: "origin".to_string(),
            visibility: "public".to_string(),
            github: RemoteGithub::default(),
        }
    }
}

fn default_remote_provider() -> String {
    "none".to_string()
}
fn default_remote_name() -> String {
    "origin".to_string()
}
fn default_visibility() -> String {
    "public".to_string()
}
fn default_rulesets() -> String {
    "auto".to_string()
}
fn default_required_check() -> String {
    "RepoWill Gates".to_string()
}
fn default_ci_runs_on() -> String {
    "ubuntu-latest".to_string()
}

// ---------------------------------------------------------------------------
// 3.11 engine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    #[serde(default)]
    pub concurrency: i64,
    #[serde(default = "default_engine_timeout")]
    pub default_timeout: String,
    #[serde(default = "default_engine_on_error")]
    pub default_on_error: String,
    #[serde(default)]
    pub require_clean_tree_for_commands: bool,
    #[serde(default = "default_true")]
    pub cache_results: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            concurrency: 0,
            default_timeout: "10m".to_string(),
            default_on_error: "block".to_string(),
            require_clean_tree_for_commands: false,
            cache_results: true,
        }
    }
}

fn default_engine_timeout() -> String {
    "10m".to_string()
}
fn default_engine_on_error() -> String {
    "block".to_string()
}

// ---------------------------------------------------------------------------
// 3.12 gates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub uses: String,
    #[serde(default)]
    pub with: serde_json::Value,
    pub stages: Vec<String>,
    #[serde(default = "default_mode_block")]
    pub mode: Mode,
    #[serde(default)]
    pub stage_modes: std::collections::BTreeMap<String, Mode>,
    #[serde(default = "default_trigger")]
    pub trigger: String,
    #[serde(default = "default_on_error_engine")]
    pub on_error: String,
    #[serde(default = "default_engine_timeout")]
    pub timeout: String,
    #[serde(default = "default_paths")]
    pub paths: GatePaths,
    #[serde(default)]
    pub run_always: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub branches: Vec<String>,
    #[serde(default = "default_only_in")]
    pub only_in: String,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatePaths {
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_trigger() -> String {
    "auto".to_string()
}
fn default_on_error_engine() -> String {
    "block".to_string()
}
fn default_paths() -> GatePaths {
    GatePaths {
        include: vec!["**".to_string()],
        exclude: vec![],
    }
}
fn default_include() -> Vec<String> {
    vec!["**".to_string()]
}
fn default_only_in() -> String {
    "any".to_string()
}

// ---------------------------------------------------------------------------
// 3.13 waivers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Waiver {
    pub id: String,
    pub target: String,
    pub reason: String,
    #[serde(default)]
    pub paths: Vec<String>,
    pub expires: String,
    #[serde(default)]
    pub request: String,
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}
fn default_mode_block() -> Mode {
    Mode::Block
}





