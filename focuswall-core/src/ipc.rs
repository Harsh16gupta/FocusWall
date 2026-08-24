//! Unix domain socket IPC protocol and framing.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use thiserror::Error;

use crate::policy::{BlockState, Policy};
use crate::storage::AuditLogEntry;

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("I/O error communicating over IPC socket: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IPC server returned error: {0}")]
    Remote(String),
}

/// Requests that can be sent by the unprivileged UI or CLI to `focuswalld`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Query daemon status, YouTube window state, active policies, and blocked domains
    GetStatus,
    /// Propose a new custom website rule to block
    AddRule {
        input: String,
        cooldown_hours: Option<u32>,
    },
    /// Request removal of a custom rule to begin the cooldown timer
    RequestRemoval {
        rule_id: i64,
        reason: Option<String>,
        cooldown_hours_override: Option<u32>,
    },
    /// Confirm and finalize removal of a custom rule after cooldown has elapsed
    ConfirmRemoval {
        rule_id: i64,
    },
    /// Cancel a pending removal request
    CancelRemovalRequest {
        rule_id: i64,
    },
    /// Retrieve recent audit log entries
    GetLogs {
        limit: Option<u32>,
    },
}

/// Responses sent by `focuswalld` to the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IpcResponse {
    Status {
        current_time: String,
        youtube_state: BlockState,
        policies: Vec<Policy>,
        blocked_domains: Vec<String>,
    },
    RuleAdded {
        policy: Policy,
    },
    RemovalRequested {
        policy: Policy,
        earliest_removal_at: String,
    },
    RemovalConfirmed {
        policy: Policy,
    },
    RemovalCancelled {
        policy: Policy,
    },
    Logs {
        entries: Vec<AuditLogEntry>,
    },
    Error {
        message: String,
    },
}

/// Maximum allowed size for a single IPC frame in bytes (64 KB) to protect against memory exhaustion.
pub const MAX_IPC_MESSAGE_SIZE: usize = 65536;

/// Sends a request over a UnixStream and reads the response frame.
pub async fn send_ipc_request(stream: &mut UnixStream, req: &IpcRequest) -> Result<IpcResponse, IpcError> {
    let mut payload = serde_json::to_string(req)?;
    payload.push('\n');

    stream.write_all(payload.as_bytes()).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    if line.len() > MAX_IPC_MESSAGE_SIZE {
        return Err(IpcError::Remote("IPC response exceeded maximum allowed frame size".to_string()));
    }

    if line.trim().is_empty() {
        return Err(IpcError::Remote("Empty response from daemon".to_string()));
    }

    let response: IpcResponse = serde_json::from_str(&line)?;
    Ok(response)
}

/// Writes a response frame to an async writer.
pub async fn write_ipc_response<W: AsyncWriteExt + Unpin>(writer: &mut W, resp: &IpcResponse) -> Result<(), IpcError> {
    let mut payload = serde_json::to_string(resp)?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_serialization_roundtrip() {
        let req = IpcRequest::AddRule {
            input: "https://www.reddit.com/r/rust".to_string(),
            cooldown_hours: Some(24),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("add_rule"));

        let deserialized: IpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, deserialized);
    }
}
