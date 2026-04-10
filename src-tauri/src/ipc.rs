use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use serde::{Deserialize, Serialize};

const IPC_ADDR: &str = "127.0.0.1:46327";

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcMessage {
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
        let mut reader = BufReader::new(stream);
        let mut line = String::new();

        let response = match reader.read_line(&mut line).await {
            Ok(n) if n > 0 => {
                if let Ok(msg) = serde_json::from_str::<IpcMessage>(line.trim()) {
                    handle_ipc_message_internal(msg, manager.clone(), app_handle.as_ref()).await
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
    
    use tauri::Emitter;
    
    match msg {
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
