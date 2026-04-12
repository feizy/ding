use tauri::Emitter;
use crate::adapter::RawAdapterEvent;
use crate::events::DingEvent;
use crate::instance::model::DingStatus;

pub fn run_adapter_monitor(
    mut adapter: Box<dyn crate::adapter::Adapter>,
    instance_id: String,
    manager: crate::SharedManager,
    app: Option<tauri::AppHandle>
) {
    if let Some(tx) = adapter.decision_sender() {
        let manager_clone = manager.clone();
        let id_clone = instance_id.clone();
        tokio::spawn(async move {
            manager_clone.lock().await.decision_channels.insert(id_clone, tx);
        });
    }

    tokio::spawn(async move {
        loop {
            match adapter.next_event().await {
                Ok(Some(event)) => {
                    let mut lock = manager.lock().await;
                    let inst = match lock.get_mut(&instance_id) {
                        Some(i) => i,
                        None => break, // Instance was removed
                    };

                    match event {
                        RawAdapterEvent::StatusChanged(status) => {
                            inst.status = status.clone();
                            inst.last_event_at = chrono::Utc::now().to_rfc3339();
                            if let Some(app) = &app {
                                let _ = app.emit("ding-event", DingEvent::StatusChanged {
                                    instance_id: instance_id.clone(),
                                    status,
                                });
                            }
                        }
                        RawAdapterEvent::ActionRequired(action) => {
                            inst.pending_action = Some(action.clone());
                            inst.status = DingStatus::ActionRequired;
                            inst.last_event_at = chrono::Utc::now().to_rfc3339();
                            if let Some(app) = &app {
                                let _ = app.emit("ding-event", DingEvent::ActionRequired {
                                    instance_id: instance_id.clone(),
                                    action,
                                });
                            }
                        }
                        RawAdapterEvent::LogLine(text, level) => {
                            let log_line = crate::instance::model::LogLine {
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                text: text.clone(),
                                level: level.clone(),
                            };
                            inst.recent_logs.push_back(log_line.clone());
                            if inst.recent_logs.len() > 100 {
                                inst.recent_logs.pop_front();
                            }
                            inst.last_event_at = chrono::Utc::now().to_rfc3339();
                            if let Some(app) = &app {
                                let _ = app.emit("ding-event", DingEvent::LogAppended {
                                    instance_id: instance_id.clone(),
                                    log: log_line,
                                });
                            }
                        }
                        RawAdapterEvent::CostUpdate(cost) => {
                            inst.cost_usd = Some(cost);
                            if let Some(app) = &app {
                                let _ = app.emit("ding-event", DingEvent::CostUpdated {
                                    instance_id: instance_id.clone(),
                                    cost_usd: cost,
                                });
                            }
                        }
                        RawAdapterEvent::ProcessExited(code) => {
                            inst.status = DingStatus::Finished;
                            inst.recent_logs.push_back(crate::instance::model::LogLine {
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                text: format!("Process exited with code {:?}", code),
                                level: crate::instance::model::LogLevel::System,
                            });
                            inst.last_event_at = chrono::Utc::now().to_rfc3339();
                            if let Some(app) = &app {
                                let _ = app.emit("ding-event", DingEvent::StatusChanged {
                                    instance_id: instance_id.clone(),
                                    status: DingStatus::Finished,
                                });
                            }
                            break;
                        }
                    }
                }
                Ok(None) => break, // stream closed
                Err(e) => {
                    let mut lock = manager.lock().await;
                    if let Some(inst) = lock.get_mut(&instance_id) {
                        inst.status = DingStatus::Error;
                        inst.recent_logs.push_back(crate::instance::model::LogLine {
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            text: format!("Adapter error: {}", e),
                            level: crate::instance::model::LogLevel::Error,
                        });
                        inst.last_event_at = chrono::Utc::now().to_rfc3339();
                        if let Some(app) = &app {
                            let _ = app.emit("ding-event", DingEvent::StatusChanged {
                                instance_id: instance_id.clone(),
                                status: DingStatus::Error,
                            });
                        }
                    }
                    break;
                }
            }
        }
        tracing::info!("Monitor for {} finished.", instance_id);
    });
}
