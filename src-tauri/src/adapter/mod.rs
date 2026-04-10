pub mod claude_code;
pub mod codex;

use async_trait::async_trait;
use crate::instance::model::*;

/// Raw events produced by any adapter
#[derive(Debug, Clone)]
pub enum RawAdapterEvent {
    StatusChanged(DingStatus),
    LogLine(String, LogLevel),
    ActionRequired(PendingAction),
    CostUpdate(f64),
    ProcessExited(Option<i32>),
}

/// Configuration for spawning an adapter
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    pub id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub allowed_tools: Option<String>,
    pub approval_mode: Option<String>,
    pub program: Option<String>,
    pub args: Vec<String>,
}

/// Unified adapter interface — each agent type implements this
#[async_trait]
pub trait Adapter: Send + 'static {
    /// Read the next event (blocks until event or process exit)
    async fn next_event(&mut self) -> anyhow::Result<Option<RawAdapterEvent>>;

    /// Send a user approval decision back to the agent
    async fn send_decision(&mut self, decision: ActionDecision) -> anyhow::Result<()>;

    /// Expose the underlying decision sender
    fn decision_sender(&self) -> Option<tokio::sync::mpsc::Sender<ActionDecision>> {
        None
    }

    /// Get the adapter type
    fn adapter_type(&self) -> AdapterType;

    /// Kill the underlying process
    async fn kill(&mut self) -> anyhow::Result<()>;
}
