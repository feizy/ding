use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use std::process::Stdio;

use super::{Adapter, AdapterConfig, RawAdapterEvent};
use crate::instance::model::*;

/// Codex JSONL event types (from codex exec stdout)
#[derive(Debug, serde::Deserialize)]
struct CodexEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(rename = "thread_id")]
    _thread_id: Option<String>,
    item: Option<CodexItem>,
    #[serde(rename = "usage")]
    _usage: Option<CodexUsage>,
    error: Option<CodexError>,
    message: Option<String>,
    // Approval fields
    call_id: Option<String>,
    command: Option<Vec<String>>,
    cwd: Option<String>,
    reason: Option<String>,
    available_decisions: Option<Vec<String>>,
    #[serde(rename = "changes")]
    _changes: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
struct CodexItem {
    #[serde(rename = "type")]
    item_type: String,
    text: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct CodexUsage {
    #[serde(rename = "input_tokens")]
    _input_tokens: u64,
    #[serde(rename = "output_tokens")]
    _output_tokens: u64,
}

#[derive(Debug, serde::Deserialize)]
struct CodexError {
    message: String,
}

/// Codex Adapter — single-channel JSONL over stdio
pub struct CodexAdapter {
    child: Child,
    lines: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    stdin: BufWriter<tokio::process::ChildStdin>,
}

impl CodexAdapter {
    pub async fn spawn(config: &AdapterConfig) -> anyhow::Result<Self> {
        let mut cmd = Command::new("codex");
        let mut args = vec!["exec".to_string(), config.prompt.clone()];

        if let Some(model) = &config.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        if let Some(mode) = &config.approval_mode {
            args.push("--approval-mode".to_string());
            args.push(mode.clone());
        }

        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().ok_or(anyhow::anyhow!("no stdout"))?;
        let stdin = child.stdin.take().ok_or(anyhow::anyhow!("no stdin"))?;

        Ok(Self {
            child,
            lines: BufReader::new(stdout).lines(),
            stdin: BufWriter::new(stdin),
        })
    }

    fn parse_event(&self, line: &str) -> Option<RawAdapterEvent> {
        let event: CodexEvent = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => {
                return Some(RawAdapterEvent::LogLine(line.to_string(), LogLevel::Info));
            }
        };

        match event.event_type.as_str() {
            "thread.started" => Some(RawAdapterEvent::StatusChanged(DingStatus::Idle)),

            "turn.started" => Some(RawAdapterEvent::StatusChanged(DingStatus::Thinking)),

            "item.started" | "item.updated" => {
                if let Some(item) = &event.item {
                    match item.item_type.as_str() {
                        "command_execution" => {
                            Some(RawAdapterEvent::StatusChanged(DingStatus::Running))
                        }
                        "reasoning" => {
                            if let Some(text) = &item.text {
                                Some(RawAdapterEvent::LogLine(
                                    text.chars().take(100).collect(),
                                    LogLevel::Info,
                                ))
                            } else {
                                Some(RawAdapterEvent::StatusChanged(DingStatus::Thinking))
                            }
                        }
                        "agent_message" => {
                            if let Some(text) = &item.text {
                                Some(RawAdapterEvent::LogLine(
                                    text.chars().take(100).collect(),
                                    LogLevel::Info,
                                ))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }

            "item.completed" => Some(RawAdapterEvent::LogLine(
                "[item completed]".to_string(),
                LogLevel::System,
            )),

            // ★ Exec approval request — this is ActionRequired
            "exec_approval_request" => {
                let command = event.command.unwrap_or_default();
                let cwd = event.cwd.unwrap_or_default();
                let decisions: Vec<ActionDecision> = event
                    .available_decisions
                    .unwrap_or_else(|| {
                        vec![
                            "approve".to_string(),
                            "deny".to_string(),
                            "abort".to_string(),
                        ]
                    })
                    .iter()
                    .filter_map(|d| match d.as_str() {
                        "approved" | "approve" => Some(ActionDecision::Approve),
                        "approved_for_session" => Some(ActionDecision::ApproveForSession),
                        "abort" => Some(ActionDecision::Abort),
                        _ => Some(ActionDecision::Deny),
                    })
                    .collect();

                Some(RawAdapterEvent::ActionRequired(PendingAction {
                    action_id: event.call_id.unwrap_or_else(|| "unknown".to_string()),
                    title: "Execution approval".to_string(),
                    message: format!("Codex wants to run: {}", command.join(" ")),
                    source_event: "exec_approval_request".to_string(),
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
                    details: Some(ActionDetails::Command {
                        command,
                        cwd,
                        reason: event.reason,
                    }),
                    raw_payload: serde_json::Value::Null,
                }))
            }

            // ★ Apply patch approval
            "apply_patch_approval_request" => {
                Some(RawAdapterEvent::ActionRequired(PendingAction {
                    action_id: event.call_id.unwrap_or_else(|| "unknown".to_string()),
                    title: "Patch approval".to_string(),
                    message: "Codex wants to modify files".to_string(),
                    source_event: "apply_patch_approval_request".to_string(),
                    kind: PendingActionKind::Choice,
                    options: vec![ActionDecision::Approve, ActionDecision::Abort]
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
                    details: Some(ActionDetails::FileDiff {
                        files: vec![], // TODO: parse changes
                    }),
                    raw_payload: serde_json::Value::Null,
                }))
            }

            "turn.completed" => Some(RawAdapterEvent::StatusChanged(DingStatus::Finished)),

            "turn.failed" => {
                if let Some(err) = &event.error {
                    Some(RawAdapterEvent::LogLine(
                        format!("Error: {}", err.message),
                        LogLevel::Error,
                    ))
                } else {
                    Some(RawAdapterEvent::StatusChanged(DingStatus::Error))
                }
            }

            "error" => {
                let msg = event.message.unwrap_or_else(|| "Unknown error".to_string());
                Some(RawAdapterEvent::LogLine(msg, LogLevel::Error))
            }

            _ => Some(RawAdapterEvent::LogLine(
                format!("[{}]", event.event_type),
                LogLevel::System,
            )),
        }
    }
}

#[async_trait]
impl Adapter for CodexAdapter {
    async fn next_event(&mut self) -> anyhow::Result<Option<RawAdapterEvent>> {
        match self.lines.next_line().await? {
            None => Ok(Some(RawAdapterEvent::ProcessExited(
                self.child.try_wait()?.map(|s| s.code().unwrap_or(-1)),
            ))),
            Some(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                Ok(self.parse_event(trimmed))
            }
        }
    }

    async fn send_decision(&mut self, decision: ActionDecision) -> anyhow::Result<()> {
        let json = match decision {
            ActionDecision::Approve => r#"{"decision":"approved"}"#,
            ActionDecision::ApproveForSession => r#"{"decision":"approved_for_session"}"#,
            ActionDecision::Deny => r#"{"decision":"deny"}"#,
            ActionDecision::Abort => r#"{"decision":"abort"}"#,
        };
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Codex
    }

    async fn kill(&mut self) -> anyhow::Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}
