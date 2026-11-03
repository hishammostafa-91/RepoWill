// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct OpResult {
    op: &'static str,
    result: serde_json::Value,
}

#[tauri::command]
fn rw_op(op: String) -> Result<OpResult, String> {
    match op.as_str() {
        "app.version" => Ok(OpResult {
            op: "app.version",
            result: json!(env!("CARGO_PKG_VERSION")),
        }),
        _ => Err(format!("unknown op: {op}")),
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
