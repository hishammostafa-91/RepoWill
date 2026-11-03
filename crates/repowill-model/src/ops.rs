// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    #[serde(rename = "project.detect")]
    ProjectDetect,
    #[serde(rename = "project.plan")]
    ProjectPlan,
    #[serde(rename = "project.init")]
    ProjectInit,
    #[serde(rename = "project.adopt")]
    ProjectAdopt,
    #[serde(rename = "project.update")]
    ProjectUpdate,
    #[serde(rename = "project.status")]
    ProjectStatus,
    #[serde(rename = "project.list")]
    ProjectList,
    #[serde(rename = "project.add")]
    ProjectAdd,
    #[serde(rename = "project.forget")]
    ProjectForget,
    #[serde(rename = "project.uninstall")]
    ProjectUninstall,

    #[serde(rename = "policy.show")]
    PolicyShow,
    #[serde(rename = "policy.validate")]
    PolicyValidate,
    #[serde(rename = "policy.diff")]
    PolicyDiff,
    #[serde(rename = "policy.apply")]
    PolicyApply,
    #[serde(rename = "policy.set")]
    PolicySet,
    #[serde(rename = "policy.explain")]
    PolicyExplain,
    #[serde(rename = "policy.schema")]
    PolicySchema,
    #[serde(rename = "policy.migrate")]
    PolicyMigrate,
    #[serde(rename = "policy.history")]
    PolicyHistory,
    #[serde(rename = "policy.revert")]
    PolicyRevert,
    #[serde(rename = "policy.lock")]
    PolicyLock,
    #[serde(rename = "policy.summary")]
    PolicySummary,

    #[serde(rename = "gate.catalog")]
    GateCatalog,
    #[serde(rename = "gate.list")]
    GateList,
    #[serde(rename = "gate.show")]
    GateShow,
    #[serde(rename = "gate.add")]
    GateAdd,
    #[serde(rename = "gate.update")]
    GateUpdate,
    #[serde(rename = "gate.remove")]
    GateRemove,
    #[serde(rename = "gate.duplicate")]
    GateDuplicate,
    #[serde(rename = "gate.move")]
    GateMove,
    #[serde(rename = "gate.set_mode")]
    GateSetMode,
    #[serde(rename = "gate.test")]
    GateTest,

    #[serde(rename = "guard.list")]
    GuardList,
    #[serde(rename = "guard.set_mode")]
    GuardSetMode,

    #[serde(rename = "check.run")]
    CheckRun,
    #[serde(rename = "fix.run")]
    FixRun,

    #[serde(rename = "report.list")]
    ReportList,
    #[serde(rename = "report.show")]
    ReportShow,

    #[serde(rename = "branch.new")]
    BranchNew,
    #[serde(rename = "branch.list")]
    BranchList,
    #[serde(rename = "branch.update")]
    BranchUpdate,

    #[serde(rename = "commit.lint")]
    CommitLint,
    #[serde(rename = "commit.create")]
    CommitCreate,

    #[serde(rename = "push.run")]
    PushRun,
    #[serde(rename = "sync.run")]
    SyncRun,

    #[serde(rename = "merge.request")]
    MergeRequest,
    #[serde(rename = "merge.status")]
    MergeStatus,
    #[serde(rename = "merge.run")]
    MergeRun,

    #[serde(rename = "tag.create")]
    TagCreate,
    #[serde(rename = "changelog.generate")]
    ChangelogGenerate,

    #[serde(rename = "request.create")]
    RequestCreate,
    #[serde(rename = "request.list")]
    RequestList,
    #[serde(rename = "request.show")]
    RequestShow,
    #[serde(rename = "request.cancel")]
    RequestCancel,
    #[serde(rename = "request.decide")]
    RequestDecide,
    #[serde(rename = "request.watch")]
    RequestWatch,

    #[serde(rename = "restore.list")]
    RestoreList,
    #[serde(rename = "restore.file")]
    RestoreFile,
    #[serde(rename = "restore.branch")]
    RestoreBranch,

    #[serde(rename = "snapshot.create")]
    SnapshotCreate,
    #[serde(rename = "snapshot.list")]
    SnapshotList,
    #[serde(rename = "snapshot.show")]
    SnapshotShow,
    #[serde(rename = "snapshot.restore")]
    SnapshotRestore,
    #[serde(rename = "snapshot.prune")]
    SnapshotPrune,

    #[serde(rename = "deps.scan")]
    DepsScan,
    #[serde(rename = "deps.check")]
    DepsCheck,
    #[serde(rename = "deps.explain")]
    DepsExplain,
    #[serde(rename = "deps.licenses")]
    DepsLicenses,

    #[serde(rename = "docs.check")]
    DocsCheck,
    #[serde(rename = "docs.map")]
    DocsMap,

    #[serde(rename = "agent.status")]
    AgentStatus,
    #[serde(rename = "agent.install")]
    AgentInstall,
    #[serde(rename = "agent.update")]
    AgentUpdate,
    #[serde(rename = "agent.remove")]
    AgentRemove,
    #[serde(rename = "agent.rules")]
    AgentRules,

    #[serde(rename = "remote.status")]
    RemoteStatus,
    #[serde(rename = "remote.auth")]
    RemoteAuth,
    #[serde(rename = "remote.connect")]
    RemoteConnect,
    #[serde(rename = "remote.create")]
    RemoteCreate,
    #[serde(rename = "remote.protect")]
    RemoteProtect,
    #[serde(rename = "remote.ci")]
    RemoteCi,
    #[serde(rename = "remote.pr")]
    RemotePr,
    #[serde(rename = "remote.disconnect")]
    RemoteDisconnect,

    #[serde(rename = "ci.run")]
    CiRun,

    #[serde(rename = "profile.list")]
    ProfileList,
    #[serde(rename = "profile.show")]
    ProfileShow,
    #[serde(rename = "profile.save")]
    ProfileSave,
    #[serde(rename = "profile.apply")]
    ProfileApply,
    #[serde(rename = "profile.export")]
    ProfileExport,
    #[serde(rename = "profile.import")]
    ProfileImport,
    #[serde(rename = "profile.delete")]
    ProfileDelete,

    #[serde(rename = "preset.list")]
    PresetList,
    #[serde(rename = "preset.show")]
    PresetShow,
    #[serde(rename = "preset.apply")]
    PresetApply,

    #[serde(rename = "hooks.status")]
    HooksStatus,
    #[serde(rename = "hooks.install")]
    HooksInstall,
    #[serde(rename = "hooks.repair")]
    HooksRepair,
    #[serde(rename = "hooks.uninstall")]
    HooksUninstall,

    #[serde(rename = "tools.list")]
    ToolsList,
    #[serde(rename = "tools.install")]
    ToolsInstall,
    #[serde(rename = "tools.update")]
    ToolsUpdate,
    #[serde(rename = "tools.verify")]
    ToolsVerify,
    #[serde(rename = "tools.remove")]
    ToolsRemove,

    #[serde(rename = "audit.log")]
    AuditLog,
    #[serde(rename = "audit.verify")]
    AuditVerify,
    #[serde(rename = "audit.export")]
    AuditExport,

    #[serde(rename = "doctor.run")]
    DoctorRun,

    #[serde(rename = "emergency.start")]
    EmergencyStart,
    #[serde(rename = "emergency.status")]
    EmergencyStatus,
    #[serde(rename = "emergency.stop")]
    EmergencyStop,

    #[serde(rename = "config.get")]
    ConfigGet,
    #[serde(rename = "config.set")]
    ConfigSet,
    #[serde(rename = "config.list")]
    ConfigList,
    #[serde(rename = "config.path")]
    ConfigPath,

    #[serde(rename = "app.version")]
    AppVersion,
    #[serde(rename = "app.update_check")]
    AppUpdateCheck,
}

impl Operation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProjectDetect => "project.detect",
            Self::ProjectPlan => "project.plan",
            Self::ProjectInit => "project.init",
            Self::ProjectAdopt => "project.adopt",
            Self::ProjectUpdate => "project.update",
            Self::ProjectStatus => "project.status",
            Self::ProjectList => "project.list",
            Self::ProjectAdd => "project.add",
            Self::ProjectForget => "project.forget",
            Self::ProjectUninstall => "project.uninstall",
            Self::PolicyShow => "policy.show",
            Self::PolicyValidate => "policy.validate",
            Self::PolicyDiff => "policy.diff",
            Self::PolicyApply => "policy.apply",
            Self::PolicySet => "policy.set",
            Self::PolicyExplain => "policy.explain",
            Self::PolicySchema => "policy.schema",
            Self::PolicyMigrate => "policy.migrate",
            Self::PolicyHistory => "policy.history",
            Self::PolicyRevert => "policy.revert",
            Self::PolicyLock => "policy.lock",
            Self::PolicySummary => "policy.summary",
            Self::GateCatalog => "gate.catalog",
            Self::GateList => "gate.list",
            Self::GateShow => "gate.show",
            Self::GateAdd => "gate.add",
            Self::GateUpdate => "gate.update",
            Self::GateRemove => "gate.remove",
            Self::GateDuplicate => "gate.duplicate",
            Self::GateMove => "gate.move",
            Self::GateSetMode => "gate.set_mode",
            Self::GateTest => "gate.test",
            Self::GuardList => "guard.list",
            Self::GuardSetMode => "guard.set_mode",
            Self::CheckRun => "check.run",
            Self::FixRun => "fix.run",
            Self::ReportList => "report.list",
            Self::ReportShow => "report.show",
            Self::BranchNew => "branch.new",
            Self::BranchList => "branch.list",
            Self::BranchUpdate => "branch.update",
            Self::CommitLint => "commit.lint",
            Self::CommitCreate => "commit.create",
            Self::PushRun => "push.run",
            Self::SyncRun => "sync.run",
            Self::MergeRequest => "merge.request",
            Self::MergeStatus => "merge.status",
            Self::MergeRun => "merge.run",
            Self::TagCreate => "tag.create",
            Self::ChangelogGenerate => "changelog.generate",
            Self::RequestCreate => "request.create",
            Self::RequestList => "request.list",
            Self::RequestShow => "request.show",
            Self::RequestCancel => "request.cancel",
            Self::RequestDecide => "request.decide",
            Self::RequestWatch => "request.watch",
            Self::RestoreList => "restore.list",
            Self::RestoreFile => "restore.file",
            Self::RestoreBranch => "restore.branch",
            Self::SnapshotCreate => "snapshot.create",
            Self::SnapshotList => "snapshot.list",
            Self::SnapshotShow => "snapshot.show",
            Self::SnapshotRestore => "snapshot.restore",
            Self::SnapshotPrune => "snapshot.prune",
            Self::DepsScan => "deps.scan",
            Self::DepsCheck => "deps.check",
            Self::DepsExplain => "deps.explain",
            Self::DepsLicenses => "deps.licenses",
            Self::DocsCheck => "docs.check",
            Self::DocsMap => "docs.map",
            Self::AgentStatus => "agent.status",
            Self::AgentInstall => "agent.install",
            Self::AgentUpdate => "agent.update",
            Self::AgentRemove => "agent.remove",
            Self::AgentRules => "agent.rules",
            Self::RemoteStatus => "remote.status",
            Self::RemoteAuth => "remote.auth",
            Self::RemoteConnect => "remote.connect",
            Self::RemoteCreate => "remote.create",
            Self::RemoteProtect => "remote.protect",
            Self::RemoteCi => "remote.ci",
            Self::RemotePr => "remote.pr",
            Self::RemoteDisconnect => "remote.disconnect",
            Self::CiRun => "ci.run",
            Self::ProfileList => "profile.list",
            Self::ProfileShow => "profile.show",
            Self::ProfileSave => "profile.save",
            Self::ProfileApply => "profile.apply",
            Self::ProfileExport => "profile.export",
            Self::ProfileImport => "profile.import",
            Self::ProfileDelete => "profile.delete",
            Self::PresetList => "preset.list",
            Self::PresetShow => "preset.show",
            Self::PresetApply => "preset.apply",
            Self::HooksStatus => "hooks.status",
            Self::HooksInstall => "hooks.install",
            Self::HooksRepair => "hooks.repair",
            Self::HooksUninstall => "hooks.uninstall",
            Self::ToolsList => "tools.list",
            Self::ToolsInstall => "tools.install",
            Self::ToolsUpdate => "tools.update",
            Self::ToolsVerify => "tools.verify",
            Self::ToolsRemove => "tools.remove",
            Self::AuditLog => "audit.log",
            Self::AuditVerify => "audit.verify",
            Self::AuditExport => "audit.export",
            Self::DoctorRun => "doctor.run",
            Self::EmergencyStart => "emergency.start",
            Self::EmergencyStatus => "emergency.status",
            Self::EmergencyStop => "emergency.stop",
            Self::ConfigGet => "config.get",
            Self::ConfigSet => "config.set",
            Self::ConfigList => "config.list",
            Self::ConfigPath => "config.path",
            Self::AppVersion => "app.version",
            Self::AppUpdateCheck => "app.update_check",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMeta {
    pub op: Operation,
    pub cli_path: &'static str,
    pub gui_route: &'static str,
    pub mutating: bool,
    pub human_only: bool,
    pub status: &'static str,
}

pub fn all_operations() -> &'static [Operation] {
    &[
        Operation::ProjectDetect,
        Operation::ProjectPlan,
        Operation::ProjectInit,
        Operation::ProjectAdopt,
        Operation::ProjectUpdate,
        Operation::ProjectStatus,
        Operation::ProjectList,
        Operation::ProjectAdd,
        Operation::ProjectForget,
        Operation::ProjectUninstall,
        Operation::PolicyShow,
        Operation::PolicyValidate,
        Operation::PolicyDiff,
        Operation::PolicyApply,
        Operation::PolicySet,
        Operation::PolicyExplain,
        Operation::PolicySchema,
        Operation::PolicyMigrate,
        Operation::PolicyHistory,
        Operation::PolicyRevert,
        Operation::PolicyLock,
        Operation::PolicySummary,
        Operation::GateCatalog,
        Operation::GateList,
        Operation::GateShow,
        Operation::GateAdd,
        Operation::GateUpdate,
        Operation::GateRemove,
        Operation::GateDuplicate,
        Operation::GateMove,
        Operation::GateSetMode,
        Operation::GateTest,
        Operation::GuardList,
        Operation::GuardSetMode,
        Operation::CheckRun,
        Operation::FixRun,
        Operation::ReportList,
        Operation::ReportShow,
        Operation::BranchNew,
        Operation::BranchList,
        Operation::BranchUpdate,
        Operation::CommitLint,
        Operation::CommitCreate,
        Operation::PushRun,
        Operation::SyncRun,
        Operation::MergeRequest,
        Operation::MergeStatus,
        Operation::MergeRun,
        Operation::TagCreate,
        Operation::ChangelogGenerate,
        Operation::RequestCreate,
        Operation::RequestList,
        Operation::RequestShow,
        Operation::RequestCancel,
        Operation::RequestDecide,
        Operation::RequestWatch,
        Operation::RestoreList,
        Operation::RestoreFile,
        Operation::RestoreBranch,
        Operation::SnapshotCreate,
        Operation::SnapshotList,
        Operation::SnapshotShow,
        Operation::SnapshotRestore,
        Operation::SnapshotPrune,
        Operation::DepsScan,
        Operation::DepsCheck,
        Operation::DepsExplain,
        Operation::DepsLicenses,
        Operation::DocsCheck,
        Operation::DocsMap,
        Operation::AgentStatus,
        Operation::AgentInstall,
        Operation::AgentUpdate,
        Operation::AgentRemove,
        Operation::AgentRules,
        Operation::RemoteStatus,
        Operation::RemoteAuth,
        Operation::RemoteConnect,
        Operation::RemoteCreate,
        Operation::RemoteProtect,
        Operation::RemoteCi,
        Operation::RemotePr,
        Operation::RemoteDisconnect,
        Operation::CiRun,
        Operation::ProfileList,
        Operation::ProfileShow,
        Operation::ProfileSave,
        Operation::ProfileApply,
        Operation::ProfileExport,
        Operation::ProfileImport,
        Operation::ProfileDelete,
        Operation::PresetList,
        Operation::PresetShow,
        Operation::PresetApply,
        Operation::HooksStatus,
        Operation::HooksInstall,
        Operation::HooksRepair,
        Operation::HooksUninstall,
        Operation::ToolsList,
        Operation::ToolsInstall,
        Operation::ToolsUpdate,
        Operation::ToolsVerify,
        Operation::ToolsRemove,
        Operation::AuditLog,
        Operation::AuditVerify,
        Operation::AuditExport,
        Operation::DoctorRun,
        Operation::EmergencyStart,
        Operation::EmergencyStatus,
        Operation::EmergencyStop,
        Operation::ConfigGet,
        Operation::ConfigSet,
        Operation::ConfigList,
        Operation::ConfigPath,
        Operation::AppVersion,
        Operation::AppUpdateCheck,
    ]
}

pub fn registry() -> Vec<OperationMeta> {
    all_operations()
        .iter()
        .map(|op| OperationMeta {
            op: *op,
            cli_path: "",
            gui_route: "",
            mutating: false,
            human_only: false,
            status: "planned",
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRequest {
    pub op: Operation,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResponse {
    pub op: Operation,
    #[serde(default)]
    pub data: serde_json::Value,
}

