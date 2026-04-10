use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The current status of a monitored instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DingStatus {
    ActionRequired = 0,
    Error = 1,
    Thinking = 2,
    Running = 3,
    Idle = 4,
    Finished = 5,
}

/// Which adapter drives this instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterType {
    ClaudeCode,
    Codex,
    Generic,
}

impl AdapterType {
    pub fn display_name(&self) -> &str {
        match self {
            AdapterType::ClaudeCode => "Claude Code",
            AdapterType::Codex => "Codex",
            AdapterType::Generic => "Custom",
        }
    }
}

/// A pending action that needs user approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    pub action_id: String,
    pub message: String,
    pub available_decisions: Vec<ActionDecision>,
    pub details: ActionDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionDecision {
    Approve,
    ApproveForSession,
    Deny,
    Abort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionDetails {
    Command {
        command: Vec<String>,
        cwd: String,
        reason: Option<String>,
    },
    FileDiff {
        files: Vec<FileDiffEntry>,
    },
    ToolUse {
        tool_name: String,
        tool_input: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiffEntry {
    pub path: String,
    pub additions: i32,
    pub deletions: i32,
}

/// A single log line from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub timestamp: String,
    pub text: String,
    pub level: LogLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Info,
    Tool,
    Error,
    System,
}

/// A monitored instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub adapter_type: AdapterType,
    pub status: DingStatus,
    pub created_at: String,
    pub last_event_at: String,
    pub pending_action: Option<PendingAction>,
    pub recent_logs: VecDeque<LogLine>,
    pub exit_code: Option<i32>,
    pub cost_usd: Option<f64>,
}

impl Instance {
    pub fn new(id: &str, name: &str, adapter_type: AdapterType) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: id.to_string(),
            name: name.to_string(),
            adapter_type,
            status: DingStatus::Idle,
            created_at: now.clone(),
            last_event_at: now,
            pending_action: None,
            recent_logs: VecDeque::with_capacity(100),
            exit_code: None,
            cost_usd: None,
        }
    }

    pub fn push_log(&mut self, text: String, level: LogLevel) {
        if self.recent_logs.len() >= 100 {
            self.recent_logs.pop_front();
        }
        self.recent_logs.push_back(LogLine {
            timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
            text,
            level,
        });
        self.last_event_at = chrono::Utc::now().to_rfc3339();
    }
}
