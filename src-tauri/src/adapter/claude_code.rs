use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use std::process::Stdio;

use super::{Adapter, AdapterConfig, RawAdapterEvent};
use crate::instance::model::*;

/// Claude Code stream-json event types
#[derive(Debug, serde::Deserialize)]
struct ClaudeStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    session_id: Option<String>,
    message: Option<ClaudeMessage>,
    result: Option<String>,
    cost_usd: Option<f64>,
    #[serde(default)]
    is_error: bool,
}

#[derive(Debug, serde::Deserialize)]
struct ClaudeMessage {
    content: Vec<ClaudeContent>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
enum ClaudeContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Option<serde_json::Value>,
    },
}

/// Claude Code Adapter
///
/// Dual-channel architecture:
///   Channel 1: stdout stream-json (NDJSON) → status monitoring
///   Channel 2: Hooks (ding-hook) ← Named Pipe → approval interception (future)
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct ClaudeCodeAdapter {
    child: Child,
    rx: tokio::sync::mpsc::Receiver<RawAdapterEvent>,
    decision_tx: tokio::sync::mpsc::Sender<ActionDecision>,
}

impl ClaudeCodeAdapter {
    pub async fn spawn(config: &AdapterConfig) -> anyhow::Result<Self> {
        let mut cmd = Command::new("claude");

        // Build args
        let mut args = vec![
            "-p".to_string(),
            config.prompt.clone(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
        ];

        if let Some(tools) = &config.allowed_tools {
            args.push("--allowedTools".to_string());
            args.push(tools.clone());
        }

        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // Inject env for hook communication
        cmd.env("DING_INSTANCE_ID", &config.id);
        cmd.env(
            "DING_HOOK_PIPE",
            format!(r"\\.\pipe\ding-hook-{}", &config.id),
        );

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().ok_or(anyhow::anyhow!("no stdout"))?;
        let mut lines = BufReader::new(stdout).lines();

        let (event_tx, rx) = tokio::sync::mpsc::channel(100);
        let (decision_tx, mut decision_rx) = tokio::sync::mpsc::channel::<ActionDecision>(10);

        // Task 1: Stdout reader
        let tx_clone = event_tx.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() { continue; }

                let event: ClaudeStreamEvent = match serde_json::from_str(trimmed) {
                    Ok(e) => e,
                    Err(_) => {
                        let _ = tx_clone.send(RawAdapterEvent::LogLine(line.to_string(), LogLevel::Info)).await;
                        continue;
                    }
                };

                if let Some(ev) = Self::parse_event(event) {
                    let _ = tx_clone.send(ev).await;
                }
            }
        });

        // Task 2: Hook Pipe Server
        let pipe_name = format!(r"\\.\pipe\ding-hook-{}", &config.id);
        let tx_hook = event_tx.clone();
        tokio::spawn(async move {
            loop {
                let mut server = match ServerOptions::new().first_pipe_instance(true).create(&pipe_name) {
                    Ok(s) => s,
                    Err(_) => {
                        // If it fails to create (e.g. collision), retry later or fallback
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                };

                if server.connect().await.is_err() {
                    continue;
                }

                let mut buf = [0u8; 4096];
                if let Ok(n) = server.read(&mut buf).await {
                    if n > 0 {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        if let Some(line) = text.lines().next() {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                                if json["type"] == "action_required" {
                                    if let Ok(action) = serde_json::from_value::<PendingAction>(json["action"].clone()) {
                                        let _ = tx_hook.send(RawAdapterEvent::ActionRequired(action)).await;
                                        
                                        // Wait for decision
                                        if let Some(decision) = decision_rx.recv().await {
                                            let resp = match decision {
                                                ActionDecision::Approve | ActionDecision::ApproveForSession => "approve\n",
                                                ActionDecision::Deny => "deny\n",
                                                ActionDecision::Abort => "abort\n",
                                            };
                                            let _ = server.write_all(resp.as_bytes()).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = server.disconnect();
            }
        });

        Ok(Self {
            child,
            rx,
            decision_tx,
        })
    }

    fn parse_event(event: ClaudeStreamEvent) -> Option<RawAdapterEvent> {
        match event.event_type.as_str() {
            "system" => Some(RawAdapterEvent::StatusChanged(DingStatus::Idle)),

            "assistant" => {
                if let Some(msg) = &event.message {
                    for content in &msg.content {
                        match content {
                            ClaudeContent::Text { text } => {
                                let preview = if text.len() > 80 {
                                    format!("{}…", &text[..80])
                                } else {
                                    text.clone()
                                };
                                return Some(RawAdapterEvent::LogLine(
                                    preview,
                                    LogLevel::Info,
                                ));
                            }
                            ClaudeContent::ToolUse { name, input, id: _ } => {
                                // Tool use → Running status + log
                                let input_summary = if let Some(cmd) =
                                    input.get("command").and_then(|v| v.as_str())
                                {
                                    cmd.to_string()
                                } else {
                                    serde_json::to_string(input)
                                        .unwrap_or_default()
                                        .chars()
                                        .take(100)
                                        .collect()
                                };

                                return Some(RawAdapterEvent::LogLine(
                                    format!("[{}] {}", name, input_summary),
                                    LogLevel::Tool,
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                Some(RawAdapterEvent::StatusChanged(DingStatus::Thinking))
            }

            "user" => {
                // tool_result events
                Some(RawAdapterEvent::StatusChanged(DingStatus::Running))
            }

            "result" => {
                if let Some(cost) = event.cost_usd {
                    // Emit cost before final status
                    // (in practice we'd buffer both, for now just return cost)
                    return Some(RawAdapterEvent::CostUpdate(cost));
                }
                if event.is_error {
                    Some(RawAdapterEvent::StatusChanged(DingStatus::Error))
                } else {
                    Some(RawAdapterEvent::StatusChanged(DingStatus::Finished))
                }
            }

            _ => None,
        }
    }
}

#[async_trait]
impl Adapter for ClaudeCodeAdapter {
    async fn next_event(&mut self) -> anyhow::Result<Option<RawAdapterEvent>> {
        match self.rx.recv().await {
            Some(ev) => Ok(Some(ev)),
            None => {
                // Channel closed means tasks exited, implying process exited
                Ok(Some(RawAdapterEvent::ProcessExited(
                    self.child.try_wait()?.map(|s| s.code().unwrap_or(-1)),
                )))
            }
        }
    }

    async fn send_decision(&mut self, decision: ActionDecision) -> anyhow::Result<()> {
        self.decision_tx.send(decision).await?;
        Ok(())
    }

    fn decision_sender(&self) -> Option<tokio::sync::mpsc::Sender<ActionDecision>> {
        Some(self.decision_tx.clone())
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::ClaudeCode
    }

    async fn kill(&mut self) -> anyhow::Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}
