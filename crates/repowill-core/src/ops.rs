// SPDX-License-Identifier: AGPL-3.0-or-later
#![deny(unsafe_code)]
#![allow(clippy::result_large_err)]
#![allow(clippy::type_complexity)]

//! Operation dispatcher used by the CLI and GUI.

use crate::config::editor;
use crate::config::strictness::semantic_diff;
use crate::config::{
    ENGINE_VERSION, builtin_policy, load_policy_file, policy_to_toml, validate, validate_source,
};
use crate::git;
use crate::project;
use crate::state::{KeyStore, StateStore};
use repowill_model::Operation;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub op_id: String,
    pub step: String,
    pub state: String,
    pub pct: u8,
    pub message: String,
    pub gate: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RwError {
    pub code: String,
    pub category: String,
    pub message: String,
    pub hint: Option<String>,
    pub details: Option<Value>,
}

impl RwError {
    pub fn exit_code(&self) -> i32 {
        match self.category.as_str() {
            "usage" => 2,
            "policy" => 3,
            "environment" => 4,
            "approval" => 5,
            "network" => 6,
            "trust" => 7,
            "cancelled" => 8,
            "conflict" => 9,
            _ => 10,
        }
    }

    pub fn planned(op: Operation) -> Self {
        Self {
            code: "RW-E1000".to_string(),
            category: "usage".to_string(),
            message: format!("operation {} is available in a later version", op.as_str()),
            hint: Some("upgrade RepoWill or use a released feature".to_string()),
            details: Some(json!({ "op": op.as_str() })),
        }
    }
}

#[derive(Clone)]
pub struct OpContext {
    pub repo_root: Option<PathBuf>,
    pub cancel: Arc<AtomicBool>,
    progress: Option<Arc<Mutex<dyn FnMut(ProgressEvent) + Send>>>,
}

impl OpContext {
    pub fn new(repo_root: Option<PathBuf>) -> Self {
        Self {
            repo_root,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: None,
        }
    }

    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: FnMut(ProgressEvent) + Send + 'static,
    {
        self.progress = Some(Arc::new(Mutex::new(callback)));
        self
    }

    pub fn emit(&self, event: ProgressEvent) {
        if let Some(callback) = &self.progress
            && let Ok(mut callback) = callback.lock()
        {
            callback(event);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

pub fn implemented_ops() -> &'static [Operation] {
    &[
        Operation::AppVersion,
        Operation::ConfigGet,
        Operation::ConfigSet,
        Operation::ConfigList,
        Operation::ConfigPath,
        Operation::PolicyShow,
        Operation::PolicyValidate,
        Operation::PolicySchema,
        Operation::PolicyExplain,
        Operation::PolicySet,
        Operation::PolicyDiff,
        Operation::AuditLog,
        Operation::AuditVerify,
        Operation::AuditExport,
        Operation::ProjectDetect,
        Operation::ProjectPlan,
        Operation::ProjectInit,
        Operation::ProjectAdopt,
        Operation::ProjectUpdate,
        Operation::ProjectList,
        Operation::ProjectAdd,
        Operation::ProjectForget,
    ]
}

pub fn execute(op: Operation, params: Value, ctx: &OpContext) -> Result<Value, RwError> {
    if ctx.is_cancelled() {
        return Err(RwError {
            code: "RW-E1001".to_string(),
            category: "cancelled".to_string(),
            message: "operation cancelled".to_string(),
            hint: None,
            details: None,
        });
    }

    let run = |params: &Value, ctx: &OpContext| -> Result<Value, RwError> {
        match op {
            Operation::AppVersion => Ok(json!({ "version": env!("CARGO_PKG_VERSION") })),
            Operation::ConfigPath => Ok(json!({ "path": app_config_path() })),
            Operation::ConfigList => config_list(),
            Operation::ConfigGet => config_get(params),
            Operation::ConfigSet => config_set(params),
            Operation::PolicyShow => policy_show(ctx, params),
            Operation::PolicyValidate => policy_validate(ctx, params),
            Operation::PolicySchema => Ok(json!({
                "schema_version": 1,
                "engine_version": ENGINE_VERSION,
                "min_repowill_version": "1.0.0"
            })),
            Operation::PolicyExplain => policy_explain(ctx, params),
            Operation::PolicySet => policy_set(ctx, params),
            Operation::PolicyDiff => policy_diff(ctx, params),
            Operation::AuditLog => audit_log(ctx, params),
            Operation::AuditVerify => audit_verify(ctx),
            Operation::AuditExport => audit_export(ctx, params),
            Operation::ProjectDetect => project_detect(ctx, params),
            Operation::ProjectPlan => project_plan(ctx, params),
            Operation::ProjectInit => project_init(ctx, params),
            Operation::ProjectAdopt => project_adopt(ctx, params),
            Operation::ProjectUpdate => project_update(ctx, params),
            Operation::ProjectList => project_list(ctx),
            Operation::ProjectAdd => project_add(ctx, params),
            Operation::ProjectForget => project_forget(ctx, params),
            _ => Err(RwError::planned(op)),
        }
    };

    ctx.emit(ProgressEvent {
        op_id: op.as_str().to_string(),
        step: "operation".to_string(),
        state: "running".to_string(),
        pct: 0,
        message: format!("running {}", op.as_str()),
        gate: None,
    });

    let result = if ctx.repo_root.is_some() {
        let lock_path = state_store(ctx).lock_path();
        if let Some(parent) = lock_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&lock_path)
        {
            let mut lock = fd_lock::RwLock::new(file);
            let _guard = lock.write().ok();
            run(&params, ctx)
        } else {
            run(&params, ctx)
        }
    } else {
        run(&params, ctx)
    };

    ctx.emit(ProgressEvent {
        op_id: op.as_str().to_string(),
        step: "operation".to_string(),
        state: "done".to_string(),
        pct: 100,
        message: format!("finished {}", op.as_str()),
        gate: None,
    });

    result
}

fn app_config_path() -> PathBuf {
    if let Ok(home) = std::env::var("REPOWILL_HOME") {
        return PathBuf::from(home).join("config.toml");
    }
    if let Some(dirs) = directories::ProjectDirs::from("org", "AfterLeads", "RepoWill") {
        return dirs.config_dir().join("config.toml");
    }
    PathBuf::from(".repowill").join("config.toml")
}

fn read_config() -> Result<toml::Value, RwError> {
    let path = app_config_path();
    if !path.exists() {
        return Ok(toml::Value::Table(Default::default()));
    }
    let text = std::fs::read_to_string(&path).map_err(|error| RwError {
        code: "RW-E1002".to_string(),
        category: "environment".to_string(),
        message: format!("cannot read config {}: {error}", path.display()),
        hint: None,
        details: None,
    })?;
    toml::from_str(&text).map_err(|error| RwError {
        code: "RW-E2000".to_string(),
        category: "policy".to_string(),
        message: format!("invalid config: {error}"),
        hint: None,
        details: None,
    })
}

fn write_config(value: &toml::Value) -> Result<(), RwError> {
    let path = app_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| RwError {
            code: "RW-E1002".to_string(),
            category: "environment".to_string(),
            message: format!("cannot create config directory: {error}"),
            hint: None,
            details: None,
        })?;
    }
    let text = toml::to_string_pretty(value).map_err(|error| RwError {
        code: "RW-E1003".to_string(),
        category: "internal".to_string(),
        message: format!("cannot serialize config: {error}"),
        hint: None,
        details: None,
    })?;
    std::fs::write(&path, text).map_err(|error| RwError {
        code: "RW-E1002".to_string(),
        category: "environment".to_string(),
        message: format!("cannot write config {}: {error}", path.display()),
        hint: None,
        details: None,
    })
}

fn config_list() -> Result<Value, RwError> {
    let config = read_config()?;
    Ok(json!({ "config": config }))
}

fn config_get(params: &Value) -> Result<Value, RwError> {
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameter `key`".to_string(),
            hint: Some("pass --key <NAME>".to_string()),
            details: None,
        })?;
    let config = read_config()?;
    let value = config.get(key).cloned();
    Ok(json!({ "key": key, "value": value }))
}

fn config_set(params: &Value) -> Result<Value, RwError> {
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameter `key`".to_string(),
            hint: Some("pass --key <NAME>".to_string()),
            details: None,
        })?;
    let value = params.get("value").cloned().unwrap_or(Value::Null);
    let mut config = read_config()?;
    let table = config.as_table_mut().ok_or_else(|| RwError {
        code: "RW-E1003".to_string(),
        category: "internal".to_string(),
        message: "config root is not a table".to_string(),
        hint: None,
        details: None,
    })?;
    table.insert(key.to_string(), json_to_toml_value(&value));
    write_config(&config)?;
    Ok(json!({ "key": key, "value": value }))
}

fn policy_path(ctx: &OpContext) -> PathBuf {
    ctx.repo_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        .join(".repowill")
        .join("policy.toml")
}

fn policy_path_from_params(ctx: &OpContext, params: &Value) -> PathBuf {
    if let Some(path) = params.get("path").and_then(Value::as_str) {
        PathBuf::from(path)
    } else {
        policy_path(ctx)
    }
}

fn policy_show(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let path = policy_path_from_params(ctx, params);
    let source = std::fs::read_to_string(&path).ok();
    let policy = if source.is_some() {
        load_policy_file(&path).map_err(policy_error)?
    } else {
        builtin_policy()
    };
    let toml = policy_to_toml(&policy).map_err(policy_error)?;
    let validation = if let Some(source) = &source {
        validate_source(source)
    } else {
        validate(&policy)
    };
    let diagnostics: Vec<Value> = validation.iter().map(diagnostic_json).collect();
    let _ = params;
    Ok(json!({
        "path": path,
        "origin": path,
        "layers": if path.exists() { json!([path]) } else { json!([]) },
        "policy": toml,
        "diagnostics": diagnostics
    }))
}

fn policy_validate(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let source = if let Some(source) = params.get("source").and_then(Value::as_str) {
        Some(source.to_string())
    } else {
        let path = policy_path_from_params(ctx, params);
        std::fs::read_to_string(&path).ok()
    };
    let diagnostics = if let Some(source) = &source {
        validate_source(source)
    } else {
        validate(&builtin_policy())
    };
    let errors: Vec<Value> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == crate::config::Severity::Error)
        .map(diagnostic_json)
        .collect();
    if !errors.is_empty() {
        return Err(RwError {
            code: errors[0]["code"].as_str().unwrap_or("RW-E2000").to_string(),
            category: "policy".to_string(),
            message: "policy validation failed".to_string(),
            hint: errors[0]["fix"].as_str().map(str::to_string),
            details: Some(
                json!({ "diagnostics": diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>() }),
            ),
        });
    }
    Ok(json!({
        "valid": true,
        "diagnostics": diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>()
    }))
}

fn policy_explain(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameter `key`".to_string(),
            hint: Some("pass --key <DOTTED.PATH>".to_string()),
            details: None,
        })?;
    let path = policy_path(ctx);
    let policy = if path.exists() {
        load_policy_file(&path).map_err(policy_error)?
    } else {
        builtin_policy()
    };
    let value =
        crate::config::resolve_path(&serde_json::to_value(&policy).map_err(policy_error)?, key)
            .cloned();
    Ok(json!({ "key": key, "value": value }))
}

fn policy_set(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameter `key`".to_string(),
            hint: None,
            details: None,
        })?;
    let value = params
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameter `value`".to_string(),
            hint: Some("pass a TOML value fragment".to_string()),
            details: None,
        })?;
    let path = policy_path(ctx);
    let mut source =
        std::fs::read_to_string(&path).unwrap_or_else(|_| "schema_version = 1\n".to_string());
    if !source.contains("schema_version") {
        source = format!("schema_version = 1\n{source}");
    }
    let updated = editor::set_key(&source, key, value).map_err(|error| RwError {
        code: "RW-E2000".to_string(),
        category: "policy".to_string(),
        message: error,
        hint: None,
        details: None,
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(policy_error)?;
    }
    std::fs::write(&path, updated).map_err(policy_error)?;
    Ok(json!({ "key": key, "path": path }))
}

fn policy_diff(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let from = params.get("from").and_then(Value::as_str);
    let to = params.get("to").and_then(Value::as_str);
    let base = if let Some(path) = from {
        load_policy_file(Path::new(path)).map_err(policy_error)?
    } else {
        builtin_policy()
    };
    let other = if let Some(path) = to {
        load_policy_file(Path::new(path)).map_err(policy_error)?
    } else {
        let path = policy_path(ctx);
        if path.exists() {
            load_policy_file(&path).map_err(policy_error)?
        } else {
            builtin_policy()
        }
    };
    let changes = semantic_diff(&base, &other);
    Ok(json!({ "changes": changes }))
}

fn diagnostic_json(diagnostic: &crate::config::Diagnostic) -> Value {
    json!({
        "code": diagnostic.code,
        "severity": diagnostic.severity.as_str(),
        "message": diagnostic.message,
        "path": diagnostic.path,
        "line": diagnostic.line,
        "column": diagnostic.column,
        "rule": diagnostic.rule,
        "fix": diagnostic.fix
    })
}

fn policy_error(error: impl std::fmt::Display) -> RwError {
    RwError {
        code: "RW-E2000".to_string(),
        category: "policy".to_string(),
        message: error.to_string(),
        hint: None,
        details: None,
    }
}

fn state_store(ctx: &OpContext) -> StateStore {
    if let Some(root) = &ctx.repo_root
        && let Ok(common) = git::common_dir(root)
    {
        return StateStore::from_common_dir(&common);
    }
    if let Some(root) = &ctx.repo_root {
        return StateStore::from_common_dir(&root.join(".git"));
    }
    StateStore::from_common_dir(&std::env::current_dir().unwrap_or_default())
}

fn audit_log(ctx: &OpContext, _params: &Value) -> Result<Value, RwError> {
    let store = state_store(ctx);
    store.ensure().map_err(policy_error)?;
    let entries = if store.audit_path().exists() {
        std::fs::read_to_string(store.audit_path())
            .map_err(policy_error)?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<Value>(line).unwrap_or(Value::Null))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Ok(json!({ "entries": entries }))
}

fn audit_verify(ctx: &OpContext) -> Result<Value, RwError> {
    let store = state_store(ctx);
    store.ensure().map_err(policy_error)?;
    let key = KeyStore::new(KeyStore::default_path())
        .load_or_create()
        .map_err(policy_error)?;
    let tampered = store.verify_audit_index(&key).map_err(policy_error)?;
    Ok(json!({ "valid": tampered.is_none(), "tampered_entry": tampered }))
}

fn audit_export(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let store = state_store(ctx);
    store.ensure().map_err(policy_error)?;
    let out = params
        .get("out")
        .and_then(Value::as_str)
        .ok_or_else(|| RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameter `out`".to_string(),
            hint: Some("pass --out <PATH>".to_string()),
            details: None,
        })?;
    if store.audit_path().exists() {
        std::fs::copy(store.audit_path(), out).map_err(policy_error)?;
    } else {
        std::fs::write(out, "").map_err(policy_error)?;
    }
    Ok(json!({ "out": out }))
}

fn project_detect(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let root = params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| ctx.repo_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    Ok(project::detect(&root))
}

fn project_plan(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let root = params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| ctx.repo_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let detection = project::detect(&root);
    let stacks: Vec<String> =
        if let Some(override_stacks) = params.get("stacks").and_then(Value::as_array) {
            override_stacks
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        } else {
            detection
                .get("stacks")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
    let project_license = params
        .get("project_license")
        .and_then(Value::as_str)
        .unwrap_or("MIT");
    let files_filter = params.get("files");
    let files = project::plan_files_filtered(&root, &stacks, project_license, files_filter);
    let file_list: Vec<Value> = files
        .iter()
        .map(|(rel, content)| {
            let existing = std::fs::read_to_string(root.join(rel)).ok();
            let diff = existing
                .as_ref()
                .map(|cur| project::unified_diff(cur, content, rel))
                .unwrap_or_else(|| project::unified_diff("", content, rel));
            json!({
                "path": rel,
                "bytes": content.len(),
                "exists": root.join(rel).exists(),
                "content": content,
                "current": existing,
                "diff": diff
            })
        })
        .collect();
    Ok(json!({ "root": root, "detection": detection, "files": file_list }))
}

fn project_init(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let root = params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| ctx.repo_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let email = params.get("email").and_then(Value::as_str).unwrap_or("");
    let resume = params
        .get("resume")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !resume && (name.is_empty() || email.is_empty()) {
        return Err(RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameters `name` and `email`".to_string(),
            hint: Some("pass --name and --email, or use --answers".to_string()),
            details: None,
        });
    }
    let project_license = params
        .get("project_license")
        .and_then(Value::as_str)
        .unwrap_or("MIT");
    let files_filter = params.get("files");
    project::apply_init(&root, name, email, resume, project_license, files_filter)
        .map_err(policy_error)
}

fn project_adopt(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let root = params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| ctx.repo_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    if !root.join(".git").exists() {
        return Err(RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "target is not a git repository; use `init` first".to_string(),
            hint: Some("run `repowill init` to create a new repository".to_string()),
            details: None,
        });
    }
    // adopt = detect + plan + apply (same as init but for existing repo)
    let detection = project::detect(&root);
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let email = params.get("email").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() || email.is_empty() {
        return Err(RwError {
            code: "RW-E1004".to_string(),
            category: "usage".to_string(),
            message: "missing required parameters `name` and `email`".to_string(),
            hint: Some("pass --name and --email, or use --answers".to_string()),
            details: None,
        });
    }
    // Only write files that don't exist yet
    let stacks: Vec<String> =
        if let Some(override_stacks) = params.get("stacks").and_then(Value::as_array) {
            override_stacks
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        } else {
            detection
                .get("stacks")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
    let project_license = params
        .get("project_license")
        .and_then(Value::as_str)
        .unwrap_or("MIT");
    let files_filter = params.get("files");
    let files = project::plan_files_filtered(&root, &stacks, project_license, files_filter);
    let mut written = 0usize;
    for (rel, content) in &files {
        let path = root.join(rel);
        let _ = crate::platform::fs::backup_file(&path);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(policy_error)?;
            }
            crate::platform::fs::atomic_write(&path, content.as_bytes()).map_err(policy_error)?;
            written += 1;
        }
    }
    // git config identity
    let git = crate::git::Git::new().map_err(policy_error)?;
    let result = git
        .run(&["config", "user.name", name], Some(&root))
        .map_err(policy_error)?;
    if result.code != Some(0) {
        return Err(policy_error(format!(
            "git config user.name failed: {}",
            result.stderr
        )));
    }
    let result = git
        .run(&["config", "user.email", email], Some(&root))
        .map_err(policy_error)?;
    if result.code != Some(0) {
        return Err(policy_error(format!(
            "git config user.email failed: {}",
            result.stderr
        )));
    }
    Ok(json!({ "root": root, "detection": detection, "written": written }))
}

fn project_update(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let root = params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| ctx.repo_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let email = params.get("email").and_then(Value::as_str).unwrap_or("");
    let project_license = params
        .get("project_license")
        .and_then(Value::as_str)
        .unwrap_or("MIT");
    let files_filter = params.get("files");
    project::apply_update(&root, name, email, project_license, files_filter).map_err(policy_error)
}

fn project_list(ctx: &OpContext) -> Result<Value, RwError> {
    let _ = ctx;
    project::project_list().map_err(policy_error)
}

fn project_add(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let root = params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| ctx.repo_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    project::project_add(&root).map_err(policy_error)
}

fn project_forget(ctx: &OpContext, params: &Value) -> Result<Value, RwError> {
    let root = params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| ctx.repo_root.clone())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    project::project_forget(&root).map_err(policy_error)
}

fn json_to_toml_value(value: &Value) -> toml::Value {
    match value {
        Value::Null => toml::Value::String(String::new()),
        Value::Bool(flag) => toml::Value::Boolean(*flag),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                toml::Value::Integer(integer)
            } else {
                toml::Value::Float(number.as_f64().unwrap_or_default())
            }
        }
        Value::String(text) => toml::Value::String(text.clone()),
        Value::Array(items) => toml::Value::Array(items.iter().map(json_to_toml_value).collect()),
        Value::Object(map) => {
            let mut table = toml::map::Map::new();
            for (key, item) in map {
                table.insert(key.clone(), json_to_toml_value(item));
            }
            toml::Value::Table(table)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;
    use serde_json::json;

    #[test]
    fn app_version_returns_version() {
        let ctx = OpContext::new(None);
        let result = execute(Operation::AppVersion, json!({}), &ctx).unwrap();
        assert_eq!(result["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn planned_operation_returns_usage_error() {
        let ctx = OpContext::new(None);
        let error = execute(Operation::ProjectStatus, json!({}), &ctx).unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.code, "RW-E1000");
    }

    #[test]
    fn policy_validate_invalid_source_returns_policy_error() {
        let ctx = OpContext::new(None);
        let error = execute(
            Operation::PolicyValidate,
            json!({ "source": "schema_version = 1\nmin_repowill_version = \"99.0.0\"" }),
            &ctx,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 3);
        assert!(
            error.details.unwrap()["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == "RW-E2018")
        );
    }

    #[test]
    fn config_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("REPOWILL_HOME", dir.path());
        }
        let ctx = OpContext::new(None);
        execute(
            Operation::ConfigSet,
            json!({ "key": "ops-test", "value": "ok" }),
            &ctx,
        )
        .unwrap();
        let result = execute(Operation::ConfigGet, json!({ "key": "ops-test" }), &ctx).unwrap();
        assert_eq!(result["value"], "ok");
        execute(Operation::ConfigList, json!({}), &ctx).unwrap();
        unsafe {
            std::env::remove_var("REPOWILL_HOME");
        }
    }
}
