// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use repowill_core::ops::{self, OpContext};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Serialize)]
struct OpResult {
    op: String,
    result: Value,
}

#[tauri::command]
fn rw_op(op: String, params: Option<Value>) -> Result<OpResult, String> {
    let ctx = OpContext::new(None);
    let operation: repowill_model::Operation =
        serde_json::from_str(&format!("\"{op}\"")).map_err(|_| format!("unknown op: {op}"))?;
    match ops::execute(operation, params.unwrap_or_else(|| json!({})), &ctx) {
        Ok(result) => Ok(OpResult { op, result }),
        Err(error) => Err(serde_json::to_string(&error).unwrap_or(error.message)),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![rw_op])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
