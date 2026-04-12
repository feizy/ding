use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use crate::claude_proxy::{build_hook_response, pending_action_from_hook};
use crate::instance::model::{ActionDecision, DingStatus, LogLevel};

const IPC_ADDR: &str = "127.0.0.1:46327";

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcMessage {
    ClaudeHookEvent {
        event_name: String,
        payload: serde_json::Value,
    },
    Claude {
        prompt: String,
        name: Option<String>,
        model: String,
        allowed_tools: Option<String>,
    },
    Codex {
        prompt: String,
        name: Option<String>,
        model: Option<String>,
        approval_mode: String,
    },
    Run {
        program: String,
        args: Vec<String>,
        name: Option<String>,
    },
    SendDecision {
        instance_id: String,
        decision: String,
    },
    List,
    Kill { id: String },
    KillAll,
    Ping,
}

pub async fn send_to_daemon(message: IpcMessage) -> anyhow::Result<String> {
    let mut client = TcpStream::connect(IPC_ADDR).await?;

    let serialized = serde_json::to_string(&message)?;
    
    // Write message
    client.write_all(serialized.as_bytes()).await?;
    // Add newline delimiter
    client.write_all(b"\n").await?;
    
    // Read response
    let mut buffer = String::new();
    let _ = client.read_to_string(&mut buffer).await;
    
    Ok(buffer)
}

pub fn is_daemon_running() -> bool {
    std::net::TcpStream::connect(IPC_ADDR)
        .is_ok()
}

pub async fn start_ipc_server_no_tauri(manager: crate::SharedManager) -> anyhow::Result<()> {
    start_ipc_server_with_manager(manager, None).await
}

pub async fn start_ipc_server(app_handle: tauri::AppHandle) -> anyhow::Result<()> {
    let manager = tauri::Manager::state::<crate::SharedManager>(&app_handle).inner().clone();
    start_ipc_server_with_manager(manager, Some(app_handle)).await
}

pub async fn start_ipc_server_with_manager(
    manager: crate::SharedManager,
    app_handle: Option<tauri::AppHandle>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(IPC_ADDR).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let manager = manager.clone();
        let app_handle = app_handle.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();

            let response = match reader.read_line(&mut line).await {
                Ok(n) if n > 0 => {
                    if let Ok(msg) = serde_json::from_str::<IpcMessage>(line.trim()) {
                        handle_ipc_message_internal(msg, manager, app_handle.as_ref()).await
                    } else {
                        "ERROR: invalid message\n".to_string()
                    }
                }
                _ => String::new(),
            };

            let mut stream = reader.into_inner();

            if !response.is_empty() {
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
    }

    #[allow(unreachable_code)]
    Ok(())
}

async fn handle_ipc_message_internal(
    msg: IpcMessage, 
    manager: crate::SharedManager, 
    app: Option<&tauri::AppHandle>
) -> String {
    tracing::info!("Received IPC: {:?}", msg);

    match msg {
        IpcMessage::ClaudeHookEvent { event_name, payload } => {
            let Some(session_id) = extract_session_id(&payload) else {
                return String::new();
            };

            let cwd = payload
                .get("cwd")
                .and_then(|value| value.as_str());
            let blocking_action = match pending_action_from_hook(&event_name, &payload) {
                Ok(action) => action,
                Err(err) => return format!("ERROR: {err}\n"),
            };

            let (instance, mut response_rx) = {
                let mut lock = manager.lock().await;
                let (instance_id, _) = lock.ensure_claude_session_instance(session_id, cwd);
                let mut response_rx = None;

                if let Some(action) = blocking_action.clone() {
                    let (tx, rx) = tokio::sync::mpsc::channel(1);
                    lock.hook_response_channels.insert(instance_id.clone(), tx);

                    let instance = lock
                        .get_mut(&instance_id)
                        .expect("instance should exist after session registration");
                    instance.pending_action = Some(action.clone());
                    instance.status = DingStatus::ActionRequired;
                    instance.current_tool_name = extract_tool_name(&payload);
                    instance.push_log(action.message.clone(), LogLevel::System);
                    response_rx = Some(rx);
                } else {
                    let instance = lock
                        .get_mut(&instance_id)
                        .expect("instance should exist after session registration");

                    apply_claude_hook_event(instance, &instance_id, &event_name, &payload);
                }

                let instance = lock
                    .get(&instance_id)
                    .expect("instance should exist after mutation")
                    .clone();
                (instance, response_rx)
            };

            if let Some(app) = app {
                emit_instance_snapshot(app, instance.clone());
            }

            if let Some(ref mut rx) = response_rx {
                let submission = match rx.recv().await {
                    Some(submission) => submission,
                    None => return "ERROR: pending Claude action channel closed\n".to_string(),
                };

                let encoded_response = build_hook_response(
                    &event_name,
                    blocking_action
                        .as_ref()
                        .map(|action| &action.raw_payload)
                        .unwrap_or(&payload),
                    submission.clone(),
                );

                let updated_instance = {
                    let mut lock = manager.lock().await;
                    lock.hook_response_channels.remove(&instance.id);

                    let instance = lock
                        .get_mut(&instance.id)
                        .expect("instance should exist while resolving action");
                    instance.pending_action = None;
                    instance.status = match &encoded_response {
                        Ok(_) => DingStatus::Running,
                        Err(_) => DingStatus::Error,
                    };
                    instance.push_log(
                        match &encoded_response {
                            Ok(_) => format!(
                                "Submitted Claude proxy response: {}",
                                submission.summary()
                            ),
                            Err(err) => format!("Claude proxy response failed: {err}"),
                        },
                        match &encoded_response {
                            Ok(_) => LogLevel::System,
                            Err(_) => LogLevel::Error,
                        },
                    );
                    instance.clone()
                };

                if let Some(app) = app {
                    emit_instance_snapshot(app, updated_instance);
                }

                return match encoded_response {
                    Ok(value) => match serde_json::to_string(&value) {
                        Ok(serialized) => format!("{serialized}\n"),
                        Err(err) => format!("ERROR: failed to serialize hook response: {err}\n"),
                    },
                    Err(err) => format!("ERROR: {err}\n"),
                };
            }

            return String::new();
        }
        IpcMessage::Ping => {
            return "PONG\n".to_string();
        }
        IpcMessage::SendDecision { instance_id, decision } => {
            let hook_sender = {
                let lock = manager.lock().await;
                lock.hook_response_channels.get(&instance_id).cloned()
            };

            if let Some(tx) = hook_sender {
                let submission = crate::instance::model::ActionSubmission::Choice {
                    selected_id: decision.clone(),
                };
                let _ = tx.send(submission).await;
            } else {
                let dec = match decision.as_str() {
                    "approve" => ActionDecision::Approve,
                    "approve_for_session" => ActionDecision::ApproveForSession,
                    "deny" => ActionDecision::Deny,
                    "abort" => ActionDecision::Abort,
                    _ => return "ERROR: invalid decision\n".to_string(),
                };

                let decision_sender = {
                    let lock = manager.lock().await;
                    lock.decision_channels.get(&instance_id).cloned()
                };

                if let Some(tx) = decision_sender {
                    let _ = tx.send(dec.clone()).await;
                }
            }

            let updated_instance = {
                let mut lock = manager.lock().await;
                let Some(instance) = lock.get_mut(&instance_id) else {
                    return format!("ERROR: instance {} not found\n", instance_id);
                };

                instance.pending_action = None;
                instance.status = DingStatus::Running;
                instance.push_log(
                    format!("Decision received via daemon: {}", decision),
                    LogLevel::System,
                );
                instance.clone()
            };

            if let Some(app) = app {
                emit_instance_snapshot(app, updated_instance);
            }

            return format!("OK: decision sent for {}\n", instance_id);
        }
        IpcMessage::List => {
            let lock = manager.lock().await;
            let instances = lock.sorted_instances();
            if instances.is_empty() {
                return "No active instances.\n".to_string();
            }
            let mut out = String::new();
            for inst in instances {
                out.push_str(&format!(
                    "[{}] {} ({:?}) — {:?}\n",
                    inst.id, inst.name, inst.adapter_type, inst.status
                ));
            }
            return out;
        }
        IpcMessage::Kill { id } => {
            let mut lock = manager.lock().await;
            if lock.remove(&id).is_some() {
                if let Some(app) = app {
                    let _ = app.emit("ding-event", crate::events::DingEvent::InstanceRemoved {
                        instance_id: id.clone(),
                    });
                }
                return format!("Killed instance {}\n", id);
            } else {
                return format!("ERROR: instance {} not found\n", id);
            }
        }
        IpcMessage::KillAll => {
            let lock = manager.lock().await;
            let ids: Vec<String> = lock.sorted_instances().iter().map(|i| i.id.clone()).collect();
            let count = ids.len();
            drop(lock);
            for id in &ids {
                let mut lock = manager.lock().await;
                lock.remove(id);
                drop(lock);
                if let Some(app) = app {
                    let _ = app.emit("ding-event", crate::events::DingEvent::InstanceRemoved {
                        instance_id: id.clone(),
                    });
                }
            }
            return format!("Killed {} instance(s)\n", count);
        }
        IpcMessage::Claude { prompt, name, model, allowed_tools } => {
            let label = name.clone().unwrap_or_else(|| {
                if prompt.len() > 30 { format!("{}\u{2026}", &prompt[..30]) } else { prompt.clone() }
            });
            
            let mut lock = manager.lock().await;
            let id = lock.create_instance(&label, crate::instance::model::AdapterType::ClaudeCode);
            drop(lock);

            let config = crate::adapter::AdapterConfig {
                id: id.clone(),
                prompt,
                model: Some(model),
                allowed_tools,
                approval_mode: None,
                program: None,
                args: vec![],
            };

            match crate::adapter::claude_code::ClaudeCodeAdapter::spawn(&config).await {
                Ok(adapter) => {
                    if let Some(app) = app {
                        if let Some(inst) = manager.lock().await.get(&id) {
                            let _ = app.emit("ding-event", crate::events::DingEvent::InstanceCreated {
                                instance: inst.clone(),
                            });
                        }
                    }
                    crate::monitor::run_adapter_monitor(
                        Box::new(adapter), id.clone(), manager.clone(), 
                        app.map(|a| a.clone())
                    );
                    return format!("OK: started Claude instance {}\n", id);
                }
                Err(e) => {
                    tracing::error!("Failed to spawn Claude: {}", e);
                    manager.lock().await.remove(&id);
                    return format!("ERROR: {e}\n");
                }
            }
        }
        IpcMessage::Codex { prompt, name, model, approval_mode } => {
            let label = name.clone().unwrap_or_else(|| {
                if prompt.len() > 30 { format!("{}\u{2026}", &prompt[..30]) } else { prompt.clone() }
            });
            
            let mut lock = manager.lock().await;
            let id = lock.create_instance(&label, crate::instance::model::AdapterType::Codex);
            drop(lock);

            let config = crate::adapter::AdapterConfig {
                id: id.clone(),
                prompt,
                model,
                allowed_tools: None,
                approval_mode: Some(approval_mode),
                program: None,
                args: vec![],
            };

            match crate::adapter::codex::CodexAdapter::spawn(&config).await {
                Ok(adapter) => {
                    if let Some(app) = app {
                        if let Some(inst) = manager.lock().await.get(&id) {
                            let _ = app.emit("ding-event", crate::events::DingEvent::InstanceCreated {
                                instance: inst.clone(),
                            });
                        }
                    }
                    crate::monitor::run_adapter_monitor(
                        Box::new(adapter), id.clone(), manager.clone(), 
                        app.map(|a| a.clone())
                    );
                    return format!("OK: started Codex instance {}\n", id);
                }
                Err(e) => {
                    tracing::error!("Failed to spawn Codex: {}", e);
                    manager.lock().await.remove(&id);
                    return format!("ERROR: {e}\n");
                }
            }
        }
        IpcMessage::Run { .. } => {
            return "ERROR: generic Run not implemented yet\n".to_string();
        }
    }
}

fn extract_session_id(payload: &serde_json::Value) -> Option<&str> {
    payload
        .get("session_id")
        .and_then(|value| value.as_str())
        .or_else(|| payload.get("sessionId").and_then(|value| value.as_str()))
}

fn apply_claude_hook_event(
    instance: &mut crate::instance::model::Instance,
    _instance_id: &str,
    event_name: &str,
    payload: &serde_json::Value,
) {
    match event_name {
        "SessionStart" => {
            instance.status = DingStatus::Idle;
            instance.current_tool_name = None;
            instance.push_log("Claude session started".to_string(), LogLevel::System);
        }
        "PreToolUse" => {
            let tool_name = payload
                .get("tool_name")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            instance.status = DingStatus::ToolCalling;
            instance.current_tool_name = Some(tool_name.to_string());
            instance.push_log(
                format!("Tool starting: {tool_name}"),
                LogLevel::Tool,
            );
        }
        "PostToolUse" => {
            instance.status = DingStatus::Running;
            instance.current_tool_name = None;
            instance.push_log(
                format!(
                    "Tool completed: {}",
                    payload
                        .get("tool_name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown")
                ),
                LogLevel::Tool,
            );
        }
        "PostToolUseFailure" => {
            instance.status = DingStatus::Error;
            instance.current_tool_name = None;
            instance.push_log("Tool failed".to_string(), LogLevel::Error);
        }
        "Notification" => {
            let message = payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("Notification received")
                .to_string();

            if notification_requires_attention(&message) {
                instance.status = DingStatus::ActionRequired;
            } else {
                instance.status = DingStatus::Running;
            }

            instance.push_log(message, LogLevel::System);
        }
        "PermissionRequest" => {
            let message = payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("Claude needs your input")
                .to_string();
            instance.status = DingStatus::ActionRequired;
            instance.push_log(message, LogLevel::System);
        }
        "Stop" => {
            instance.status = DingStatus::Idle;
            instance.current_tool_name = None;
            instance.push_log("Claude turn completed".to_string(), LogLevel::System);
        }
        "SessionEnd" => {
            instance.status = DingStatus::Finished;
            instance.current_tool_name = None;
            instance.push_log("Claude session ended".to_string(), LogLevel::System);
        }
        _ => {
            instance.push_log(
                format!("Claude hook event: {event_name}"),
                LogLevel::System,
            );
        }
    }
}

fn notification_requires_attention(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("needs your attention")
        || lower.contains("permission")
        || lower.contains("input")
        || lower.contains("confirm")
        || lower.contains("approval")
        || lower.contains("select")
        || lower.contains("choose")
        || lower.contains("pick")
}

fn extract_tool_name(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn emit_instance_snapshot(app: &tauri::AppHandle, instance: crate::instance::model::Instance) {
    let _ = app.emit(
        "ding-event",
        crate::events::DingEvent::InstanceCreated {
            instance: instance.clone(),
        },
    );

    if let Some(action) = instance.pending_action {
        let _ = app.emit(
            "ding-event",
            crate::events::DingEvent::ActionRequired {
                instance_id: instance.id,
                action,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::notification_requires_attention;

    #[test]
    fn notification_requires_attention_for_native_attention_prompt() {
        assert!(notification_requires_attention("Claude Code needs your attention"));
    }
}
