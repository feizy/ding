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
    pub created_at: String,
    pub last_event_at: String,
    pub pending_action: Option<PendingAction>,
    pub recent_logs: Vec<LogLine>,
    pub exit_code: Option<i32>,
    pub cost_usd: Option<f64>,
}

#[derive(serde::Serialize)]
pub struct InstanceListResponse {
    pub instances: Vec<InstanceInfo>,
    pub total: usize,
    pub action_required_count: usize,
    pub primary_status: DingStatus,
}

fn instance_to_info(inst: &Instance) -> InstanceInfo {
    InstanceInfo {
        id: inst.id.clone(),
        name: inst.name.clone(),
        adapter_type: inst.adapter_type.clone(),
        adapter_label: inst.adapter_type.display_name().to_string(),
        status: inst.status.clone(),
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

/// Create a Claude Code instance (demo: creates with mock data for now)
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

/// Create a Codex instance (demo)
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
    let dec = match decision.as_str() {
        "approve" => ActionDecision::Approve,
        "approve_for_session" => ActionDecision::ApproveForSession,
        "deny" => ActionDecision::Deny,
        "abort" => ActionDecision::Abort,
        _ => return Err("Invalid decision".to_string()),
    };

    let mut mgr = manager.lock().await;

    // Send the decision via channel
    if let Some(tx) = mgr.decision_channels.get(&instance_id) {
        let _ = tx.send(dec).await;
    }

    if let Some(inst) = mgr.get_mut(&instance_id) {
        let decision_label = decision.clone();
        inst.pending_action = None;
        inst.status = DingStatus::Running;
        inst.push_log(format!("User decision: {}", decision_label), LogLevel::System);
        let _ = app.emit("ding-update", "decision_sent");
        Ok(())
    } else {
        Err(format!("Instance {} not found", instance_id))
    }
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
