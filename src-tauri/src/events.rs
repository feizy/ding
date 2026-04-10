use serde::{Deserialize, Serialize};
use crate::instance::model::*;

/// Events emitted to the frontend via Tauri's event system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DingEvent {
    /// An instance's status changed
    StatusChanged {
        instance_id: String,
        status: DingStatus,
    },
    /// An instance requires user action
    ActionRequired {
        instance_id: String,
        action: PendingAction,
    },
    /// A new log line was added
    LogAppended {
        instance_id: String,
        log: LogLine,
    },
    /// An instance was created
    InstanceCreated {
        instance: Instance,
    },
    /// An instance was removed
    InstanceRemoved {
        instance_id: String,
    },
    /// Cost update
    CostUpdated {
        instance_id: String,
        cost_usd: f64,
    },
}
