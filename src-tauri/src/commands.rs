use tauri::{AppHandle, Emitter, Window};
use crate::SharedManager;
use crate::instance::model::*;

/// Serializable instance info for the frontend
#[derive(serde::Serialize)]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub adapter_type: AdapterType,
    pub adapter_label: String,
    pub status: DingStatus,
    pub current_tool_name: Option<String>,
    pub created_at: String,
    pub last_event_at: String,
    pub pending_action: Option<PendingAction>,
    pub recent_logs: Vec<LogLine>,
    pub exit_code: Option<i32>,
    pub cost_usd: Option<f64>,
}

fn instance_to_info(inst: &Instance) -> InstanceInfo {
    InstanceInfo {
        id: inst.id.clone(),
        name: inst.name.clone(),
        adapter_type: inst.adapter_type.clone(),
        adapter_label: inst.adapter_type.display_name().to_string(),
        status: inst.status.clone(),
        current_tool_name: inst.current_tool_name.clone(),
        created_at: inst.created_at.clone(),
        last_event_at: inst.last_event_at.clone(),
        pending_action: inst.pending_action.clone(),
        recent_logs: inst.recent_logs.iter().cloned().collect(),
        exit_code: inst.exit_code,
        cost_usd: inst.cost_usd,
    }
}

/// Get all instances sorted by priority
#[tauri::command]
pub async fn get_instances(
    manager: tauri::State<'_, SharedManager>,
) -> Result<Vec<InstanceInfo>, String> {
    let mgr = manager.lock().await;
    let sorted = mgr.sorted_instances();
    let infos: Vec<InstanceInfo> = sorted.iter().map(|i| instance_to_info(i)).collect();
    Ok(infos)
}

#[tauri::command]
pub async fn create_claude_instance(
    manager: tauri::State<'_, SharedManager>,
    app: AppHandle,
    prompt: String,
    name: Option<String>,
) -> Result<String, String> {
    let mut mgr = manager.lock().await;
    let display_name = name.unwrap_or_else(|| {
        if prompt.len() > 24 {
            format!("{}…", &prompt[..24])
        } else {
            prompt.clone()
        }
    });
    let id = mgr.create_instance(&display_name, AdapterType::ClaudeCode);

    // Update status to Thinking (in real impl, this comes from the adapter)
    if let Some(inst) = mgr.get_mut(&id) {
        inst.status = DingStatus::Thinking;
        inst.push_log(format!("Prompt: {}", prompt), LogLevel::System);
        inst.push_log("Connecting to Claude Code...".to_string(), LogLevel::Info);
    }

    // Emit event to frontend
    let _ = app.emit("ding-update", "instance_created");

    Ok(id)
}

#[tauri::command]
pub async fn create_codex_instance(
    manager: tauri::State<'_, SharedManager>,
    app: AppHandle,
    prompt: String,
    name: Option<String>,
) -> Result<String, String> {
    let mut mgr = manager.lock().await;
    let display_name = name.unwrap_or_else(|| {
        if prompt.len() > 24 {
            format!("{}…", &prompt[..24])
        } else {
            prompt.clone()
        }
    });
    let id = mgr.create_instance(&display_name, AdapterType::Codex);

    if let Some(inst) = mgr.get_mut(&id) {
        inst.status = DingStatus::Thinking;
        inst.push_log(format!("Prompt: {}", prompt), LogLevel::System);
        inst.push_log("Connecting to Codex...".to_string(), LogLevel::Info);
    }

    let _ = app.emit("ding-update", "instance_created");

    Ok(id)
}

#[tauri::command]
pub async fn send_decision(
    manager: tauri::State<'_, SharedManager>,
    app: AppHandle,
    instance_id: String,
    decision: String,
) -> Result<(), String> {
    submit_action_impl(
        manager.inner().clone(),
        &app,
        instance_id,
        ActionSubmission::Choice {
            selected_id: decision,
        },
    )
    .await
}

#[tauri::command]
pub async fn submit_action(
    manager: tauri::State<'_, SharedManager>,
    app: AppHandle,
    instance_id: String,
    submission: ActionSubmission,
) -> Result<(), String> {
    submit_action_impl(manager.inner().clone(), &app, instance_id, submission).await
}

/// Kill an instance
#[tauri::command]
pub async fn kill_instance(
    manager: tauri::State<'_, SharedManager>,
    app: AppHandle,
    instance_id: String,
) -> Result<(), String> {
    let mut mgr = manager.lock().await;
    mgr.remove(&instance_id);
    let _ = app.emit("ding-update", "instance_removed");
    Ok(())
}

/// Resize widget window (capsule ↔ panel)
#[tauri::command]
pub async fn resize_widget(
    window: Window,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let size = tauri::LogicalSize::new(width, height);
    window
        .set_size(tauri::Size::Logical(size))
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn submit_action_impl(
    manager: SharedManager,
    app: &AppHandle,
    instance_id: String,
    submission: ActionSubmission,
) -> Result<(), String> {
    let hook_sender = {
        let mgr = manager.lock().await;
        if mgr.get(&instance_id).is_none() {
            return Err(format!("Instance {} not found", instance_id));
        }
        mgr.hook_response_channels.get(&instance_id).cloned()
    };

    if let Some(tx) = hook_sender {
        tx.send(submission.clone())
            .await
            .map_err(|_| "Failed to forward action to Claude proxy".to_string())?;
    } else {
        let decision = submission
            .as_legacy_decision()
            .ok_or_else(|| "This action does not support the submitted response type".to_string())?;

        let decision_sender = {
            let mgr = manager.lock().await;
            mgr.decision_channels.get(&instance_id).cloned()
        };

        let Some(tx) = decision_sender else {
            return Err("No active action channel for this instance".to_string());
        };

        tx.send(decision)
            .await
            .map_err(|_| "Failed to forward action to adapter".to_string())?;
    }

    let updated_instance = {
        let mut mgr = manager.lock().await;
        let Some(inst) = mgr.get_mut(&instance_id) else {
            return Err(format!("Instance {} not found", instance_id));
        };

        inst.pending_action = None;
        inst.status = DingStatus::Running;
        inst.push_log(
            format!("Submitted action response: {}", submission.summary()),
            LogLevel::System,
        );
        inst.clone()
    };

    let _ = app.emit(
        "ding-event",
        crate::events::DingEvent::InstanceCreated {
            instance: updated_instance,
        },
    );

    Ok(())
}
