// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use tokio::net::UnixStream;
use tauri::command;

use focuswall_core::{
    send_ipc_request, AuditLogEntry, DaemonConfig, IpcRequest, IpcResponse, Policy,
};

async fn get_connected_socket() -> Result<UnixStream, String> {
    let cfg = DaemonConfig::load_default();
    let candidates = [
        cfg.socket_path.clone(),
        PathBuf::from("/run/focuswall/focuswall.sock"),
        PathBuf::from("/tmp/focuswall.sock"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            if let Ok(stream) = UnixStream::connect(candidate).await {
                return Ok(stream);
            }
        }
    }

    Err("Could not connect to focuswalld Unix domain socket at /run/focuswall/focuswall.sock. Ensure focuswalld service is running.".to_string())
}

#[command]
async fn get_status() -> Result<serde_json::Value, String> {
    let mut stream = get_connected_socket().await?;
    let resp = send_ipc_request(&mut stream, &IpcRequest::GetStatus)
        .await
        .map_err(|e| e.to_string())?;

    match resp {
        IpcResponse::Status {
            current_time,
            youtube_state,
            policies,
            blocked_domains,
        } => Ok(serde_json::json!({
            "current_time": current_time,
            "youtube_state": youtube_state,
            "policies": policies,
            "blocked_domains": blocked_domains,
        })),
        IpcResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}

#[command]
async fn add_rule(input: String, cooldown_hours: u32) -> Result<Policy, String> {
    let mut stream = get_connected_socket().await?;
    let resp = send_ipc_request(
        &mut stream,
        &IpcRequest::AddRule {
            input,
            cooldown_hours: Some(cooldown_hours),
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    match resp {
        IpcResponse::RuleAdded { policy } => Ok(policy),
        IpcResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}

#[command]
async fn request_removal(rule_id: i64, reason: Option<String>) -> Result<Policy, String> {
    let mut stream = get_connected_socket().await?;
    let resp = send_ipc_request(
        &mut stream,
        &IpcRequest::RequestRemoval {
            rule_id,
            reason,
            cooldown_hours_override: None,
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    match resp {
        IpcResponse::RemovalRequested { policy, .. } => Ok(policy),
        IpcResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}

#[command]
async fn confirm_removal(rule_id: i64) -> Result<Policy, String> {
    let mut stream = get_connected_socket().await?;
    let resp = send_ipc_request(&mut stream, &IpcRequest::ConfirmRemoval { rule_id })
        .await
        .map_err(|e| e.to_string())?;

    match resp {
        IpcResponse::RemovalConfirmed { policy } => Ok(policy),
        IpcResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}

#[command]
async fn cancel_removal(rule_id: i64) -> Result<Policy, String> {
    let mut stream = get_connected_socket().await?;
    let resp = send_ipc_request(&mut stream, &IpcRequest::CancelRemovalRequest { rule_id })
        .await
        .map_err(|e| e.to_string())?;

    match resp {
        IpcResponse::RemovalCancelled { policy } => Ok(policy),
        IpcResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}

#[command]
async fn get_logs(limit: Option<u32>) -> Result<Vec<AuditLogEntry>, String> {
    let mut stream = get_connected_socket().await?;
    let resp = send_ipc_request(&mut stream, &IpcRequest::GetLogs { limit })
        .await
        .map_err(|e| e.to_string())?;

    match resp {
        IpcResponse::Logs { entries } => Ok(entries),
        IpcResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            add_rule,
            request_removal,
            confirm_removal,
            cancel_removal,
            get_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running FocusWall Tauri application");
}
