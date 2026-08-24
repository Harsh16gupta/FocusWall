use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

use focuswall_core::{
    evaluate_youtube_state, normalize_domain_input, send_ipc_request, write_ipc_response,
    Database, IpcRequest, IpcResponse,
};

async fn mock_ipc_server(socket_path: PathBuf, db: Arc<Mutex<Database>>) {
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path).unwrap();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let db_clone = Arc::clone(&db);
            tokio::spawn(async move {
                let (reader, mut writer) = stream.split();
                let mut buf_reader = tokio::io::BufReader::new(reader);
                let mut line = String::new();

                while let Ok(n) = tokio::io::AsyncBufReadExt::read_line(&mut buf_reader, &mut line).await {
                    if n == 0 {
                        break;
                    }
                    if let Ok(req) = serde_json::from_str::<IpcRequest>(&line) {
                        let resp = match req {
                            IpcRequest::GetStatus => {
                                let db_g = db_clone.lock().await;
                                let now = chrono::Local::now();
                                let yt = evaluate_youtube_state(&now);
                                let policies = db_g.get_active_policies().unwrap_or_default();
                                let blocked = db_g.get_blocked_domains(&now).unwrap_or_default();
                                IpcResponse::Status {
                                    current_time: now.to_rfc3339(),
                                    youtube_state: yt,
                                    policies,
                                    blocked_domains: blocked,
                                }
                            }
                            IpcRequest::AddRule { input, cooldown_hours } => {
                                let norm = normalize_domain_input(&input).unwrap();
                                let db_g = db_clone.lock().await;
                                let policy = db_g.add_custom_rule(
                                    &norm.root_domain,
                                    &norm.domains,
                                    cooldown_hours.unwrap_or(24),
                                ).unwrap();
                                IpcResponse::RuleAdded { policy }
                            }
                            IpcRequest::RequestRemoval { rule_id, reason, cooldown_hours_override } => {
                                let db_g = db_clone.lock().await;
                                match db_g.request_removal(rule_id, reason.as_deref(), cooldown_hours_override) {
                                    Ok(policy) => {
                                        let era = policy.earliest_removal_at.clone().unwrap_or_default();
                                        IpcResponse::RemovalRequested { policy, earliest_removal_at: era }
                                    }
                                    Err(e) => IpcResponse::Error { message: e.to_string() },
                                }
                            }
                            IpcRequest::ConfirmRemoval { rule_id } => {
                                let db_g = db_clone.lock().await;
                                let now = chrono::Utc::now();
                                match db_g.confirm_removal(rule_id, &now) {
                                    Ok(policy) => IpcResponse::RemovalConfirmed { policy },
                                    Err(e) => IpcResponse::Error { message: e.to_string() },
                                }
                            }
                            IpcRequest::CancelRemovalRequest { rule_id } => {
                                let db_g = db_clone.lock().await;
                                match db_g.cancel_removal_request(rule_id) {
                                    Ok(policy) => IpcResponse::RemovalCancelled { policy },
                                    Err(e) => IpcResponse::Error { message: e.to_string() },
                                }
                            }
                            IpcRequest::GetLogs { limit } => {
                                let db_g = db_clone.lock().await;
                                let entries = db_g.get_audit_logs(limit.unwrap_or(20)).unwrap_or_default();
                                IpcResponse::Logs { entries }
                            }
                        };
                        let _ = write_ipc_response(&mut writer, &resp).await;
                    }
                    line.clear();
                }
            });
        }
    });
}

#[tokio::test]
async fn test_ipc_socket_full_lifecycle() {
    let temp_sock = NamedTempFile::new().unwrap();
    let sock_path = temp_sock.path().to_path_buf();

    let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
    mock_ipc_server(sock_path.clone(), Arc::clone(&db)).await;

    // Allow socket to bind
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut stream = UnixStream::connect(&sock_path).await.expect("connects to socket");

    // 1. Send GetStatus
    let resp = send_ipc_request(&mut stream, &IpcRequest::GetStatus).await.unwrap();
    match resp {
        IpcResponse::Status { policies, .. } => {
            assert_eq!(policies.len(), 1);
            assert_eq!(policies[0].name, "youtube");
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 2. Add custom rule for reddit
    let add_resp = send_ipc_request(
        &mut stream,
        &IpcRequest::AddRule {
            input: "https://www.reddit.com/r/rust".to_string(),
            cooldown_hours: Some(24),
        },
    ).await.unwrap();

    let rule_id = match add_resp {
        IpcResponse::RuleAdded { policy } => {
            assert_eq!(policy.name, "reddit.com");
            policy.id.unwrap()
        }
        other => panic!("Unexpected response: {:?}", other),
    };

    // 3. Request removal
    let req_rem_resp = send_ipc_request(
        &mut stream,
        &IpcRequest::RequestRemoval {
            rule_id,
            reason: Some("Test removal".to_string()),
            cooldown_hours_override: None,
        },
    ).await.unwrap();

    match req_rem_resp {
        IpcResponse::RemovalRequested { policy, .. } => {
            assert_eq!(policy.status, focuswall_core::PolicyStatus::RemovalPending);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 4. Confirm removal prematurely -> Must return error
    let early_confirm_resp = send_ipc_request(
        &mut stream,
        &IpcRequest::ConfirmRemoval { rule_id },
    ).await.unwrap();

    assert!(matches!(early_confirm_resp, IpcResponse::Error { .. }));

    // 5. Cancel removal
    let cancel_resp = send_ipc_request(
        &mut stream,
        &IpcRequest::CancelRemovalRequest { rule_id },
    ).await.unwrap();

    match cancel_resp {
        IpcResponse::RemovalCancelled { policy } => {
            assert_eq!(policy.status, focuswall_core::PolicyStatus::Active);
        }
        other => panic!("Unexpected response: {:?}", other),
    }
}
