use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use crate::instance::model::{ActionDecision, ActionDetails, DingStatus, LogLevel, PendingAction};

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

            let (instance, pending_decision_rx) = {
                let mut lock = manager.lock().await;
                let (instance_id, _) = lock.ensure_claude_session_instance(session_id, cwd);
                let pending_decision_rx = if event_name == "PreToolUse" {
                    let (tx, rx) = tokio::sync::mpsc::channel(1);
                    lock.decision_channels.insert(instance_id.clone(), tx);
                    Some(rx)
                } else {
                    None
                };
                let instance = lock
                    .get_mut(&instance_id)
                    .expect("instance should exist after session registration");

                apply_claude_hook_event(instance, &instance_id, &event_name, &payload);
                (instance.clone(), pending_decision_rx)
            };

            if let Some(app) = app {
                emit_instance_snapshot(app, instance.clone());
            }

            if let Some(mut decision_rx) = pending_decision_rx {
                let decision = decision_rx.recv().await.unwrap_or(ActionDecision::Deny);

                let updated_instance = {
                    let mut lock = manager.lock().await;
                    lock.decision_channels.remove(&instance.id);
                    let instance = lock
                        .get_mut(&instance.id)
                        .expect("instance should still exist when resolving decision");
                    instance.pending_action = None;
                    instance.status = DingStatus::Running;
                    instance.push_log(
                        format!("Claude tool decision: {:?}", decision),
                        LogLevel::System,
                    );
                    instance.clone()
                };

                if let Some(app) = app {
                    emit_instance_snapshot(app, updated_instance);
                }

                return format!(
                    "{}\n",
                    crate::claude_hooks::pre_tool_use_decision_response(decision)
                );
            }

            return String::new();
        }
        IpcMessage::Ping => {
            return "PONG\n".to_string();
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
            let mut lock = manager.lock().await;
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
    instance_id: &str,
    event_name: &str,
    payload: &serde_json::Value,
) {
    match event_name {
        "SessionStart" => {
            instance.status = DingStatus::Idle;
            instance.push_log("Claude session started".to_string(), LogLevel::System);
        }
        "PreToolUse" => {
            let tool_name = payload
                .get("tool_name")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");

            let action = PendingAction {
                action_id: format!("{}-{}", instance_id, chrono::Utc::now().timestamp_millis()),
                message: format!("Claude wants to use tool: {tool_name}"),
                available_decisions: vec![
                    ActionDecision::Approve,
                    ActionDecision::Deny,
                    ActionDecision::Abort,
                ],
                details: ActionDetails::ToolUse {
                    tool_name: tool_name.to_string(),
                    tool_input: payload
                        .get("tool_input")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({})),
                },
            };

            instance.pending_action = Some(action);
            instance.status = DingStatus::ActionRequired;
            instance.push_log(
                format!("Tool awaiting approval: {tool_name}"),
                LogLevel::Tool,
            );
        }
        "PostToolUse" => {
            instance.status = DingStatus::Running;
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
            instance.push_log("Tool failed".to_string(), LogLevel::Error);
        }
        "Notification" => {
            instance.status = DingStatus::Running;
            instance.push_log(
                payload
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Notification received")
                    .to_string(),
                LogLevel::System,
            );
        }
        "Stop" => {
            instance.status = DingStatus::Idle;
            instance.push_log("Claude turn completed".to_string(), LogLevel::System);
        }
        "SessionEnd" => {
            instance.status = DingStatus::Finished;
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
