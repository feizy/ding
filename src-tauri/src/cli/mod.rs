pub mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ding", version, about = "Desktop Interactive Node Guard — AI Agent Monitor")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Launch and monitor a Claude Code agent
    Claude {
        /// Arguments passed through to the native Claude CLI
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Launch and monitor a Codex agent
    Codex {
        /// The prompt / task for Codex
        #[arg(required = true)]
        prompt: String,
        /// Display name for this instance
        #[arg(short, long)]
        name: Option<String>,
        /// Model to use (o3, gpt-4.1, etc.)
        #[arg(long)]
        model: Option<String>,
        /// Approval mode: suggest | auto-edit | full-auto
        #[arg(long, default_value = "suggest")]
        approval_mode: String,
    },
    /// Launch and monitor a generic program (requires ding-sdk)
    Run {
        /// Program to execute
        #[arg(required = true)]
        program: String,
        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// Display name for this instance
        #[arg(short, long)]
        name: Option<String>,
    },
    /// List all active instances
    List,
    /// Kill a specific instance
    Kill {
        /// Instance ID (short hex, e.g. "0001")
        id: String,
    },
    /// Kill all instances
    KillAll,
    /// Internal: relay a Claude hook event back to the local daemon
    #[command(hide = true)]
    HookRelay {
        event_name: String,
    },
    /// Internal: resolve a pending instance decision through the daemon
    #[command(hide = true)]
    Decide {
        instance_id: String,
        decision: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::Parser;

    #[test]
    fn claude_subcommand_allows_empty_args() {
        let cli = Cli::try_parse_from(["ding", "claude"]).unwrap();

        match cli.command.unwrap() {
            Commands::Claude { args } => assert!(args.is_empty()),
            _ => panic!("expected Claude command"),
        }
    }
}
