use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The current status of a monitored instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DingStatus {
    ActionRequired = 0,
    Error = 1,
    Thinking = 2,
    ToolCalling = 3,
    Running = 4,
    Idle = 5,
    Finished = 6,
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
    pub title: String,
    pub message: String,
    pub source_event: String,
    pub kind: PendingActionKind,
    pub options: Vec<ActionOption>,
    pub input: Option<ActionInputSpec>,
    pub form: Option<ActionFormSpec>,
    pub details: Option<ActionDetails>,
    #[serde(skip_serializing, skip_deserializing, default)]
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PendingActionKind {
    Choice,
    Input,
    Form,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub style: ActionOptionStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionOptionStyle {
    Primary,
    Secondary,
    Danger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInputSpec {
    pub placeholder: Option<String>,
    pub submit_label: String,
    pub multiline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionFormSpec {
    pub submit_label: String,
    pub fields: Vec<ActionFormField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionFormField {
    pub id: String,
    pub label: String,
    pub field_type: ActionFormFieldType,
    pub placeholder: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionFormFieldType {
    Text,
    Multiline,
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionSubmission {
    Choice {
        selected_id: String,
    },
    Input {
        value: String,
    },
    Form {
        values: serde_json::Map<String, serde_json::Value>,
    },
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

impl ActionDecision {
    pub fn submission_id(&self) -> &'static str {
        match self {
            ActionDecision::Approve => "approve",
            ActionDecision::ApproveForSession => "approve_for_session",
            ActionDecision::Deny => "deny",
            ActionDecision::Abort => "abort",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ActionDecision::Approve => "Approve",
            ActionDecision::ApproveForSession => "Always allow",
            ActionDecision::Deny => "Deny",
            ActionDecision::Abort => "Abort",
        }
    }

    pub fn style(&self) -> ActionOptionStyle {
        match self {
            ActionDecision::Approve => ActionOptionStyle::Primary,
            ActionDecision::ApproveForSession => ActionOptionStyle::Secondary,
            ActionDecision::Deny => ActionOptionStyle::Danger,
            ActionDecision::Abort => ActionOptionStyle::Secondary,
        }
    }

    pub fn from_submission_id(value: &str) -> Option<Self> {
        match value {
            "approve" => Some(ActionDecision::Approve),
            "approve_for_session" => Some(ActionDecision::ApproveForSession),
            "deny" => Some(ActionDecision::Deny),
            "abort" => Some(ActionDecision::Abort),
            _ => None,
        }
    }
}

impl ActionSubmission {
    pub fn as_legacy_decision(&self) -> Option<ActionDecision> {
        match self {
            ActionSubmission::Choice { selected_id } => {
                ActionDecision::from_submission_id(selected_id)
            }
            ActionSubmission::Input { .. } | ActionSubmission::Form { .. } => None,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            ActionSubmission::Choice { selected_id } => format!("choice:{selected_id}"),
            ActionSubmission::Input { value } => format!("input:{}", value.trim()),
            ActionSubmission::Form { values } => {
                let keys: Vec<_> = values.keys().cloned().collect();
                format!("form:{}", keys.join(","))
            }
        }
    }
}

impl PendingAction {
    pub fn legacy_decision_action(
        action_id: String,
        title: impl Into<String>,
        message: impl Into<String>,
        source_event: impl Into<String>,
        decisions: Vec<ActionDecision>,
        details: Option<ActionDetails>,
    ) -> Self {
        Self {
            action_id,
            title: title.into(),
            message: message.into(),
            source_event: source_event.into(),
            kind: PendingActionKind::Choice,
            options: decisions
                .into_iter()
                .map(|decision| ActionOption {
                    id: decision.submission_id().to_string(),
                    label: decision.label().to_string(),
                    description: None,
                    style: decision.style(),
                })
                .collect(),
            input: None,
            form: None,
            details,
            raw_payload: serde_json::Value::Null,
        }
    }
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
    pub current_tool_name: Option<String>,
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
            current_tool_name: None,
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

#[cfg(test)]
mod tests {
    use super::DingStatus;

    #[test]
    fn tool_calling_should_sort_between_thinking_and_running() {
        let mut statuses = vec![
            DingStatus::Running,
            DingStatus::Thinking,
            DingStatus::ToolCalling,
        ];

        statuses.sort();

        assert_eq!(
            statuses,
            vec![
                DingStatus::Thinking,
                DingStatus::ToolCalling,
                DingStatus::Running,
            ]
        );
    }
}
