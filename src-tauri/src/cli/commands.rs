/// CLI command handlers
use super::Commands;
use crate::ipc::{self, IpcMessage};

pub async fn handle_command(cmd: Commands) {
    if !ipc::is_daemon_running() {
        println!("Ding daemon not running. Starting in background...");
        // Auto-start the daemon
        let exe_path = std::env::current_exe().unwrap_or_default();
        let _ = std::process::Command::new(exe_path)
            .spawn();
        
        // Wait a bit for it to initialize
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    let msg = match cmd {
        Commands::Claude { prompt, name, model, allowed_tools } => {
            IpcMessage::Claude { prompt, name, model, allowed_tools }
        }
        Commands::Codex { prompt, name, model, approval_mode } => {
            IpcMessage::Codex { prompt, name, model, approval_mode }
        }
        Commands::Run { program, args, name } => {
            IpcMessage::Run { program, args, name }
        }
        Commands::List => IpcMessage::List,
        Commands::Kill { id } => IpcMessage::Kill { id },
        Commands::KillAll => IpcMessage::KillAll,
    };

    match ipc::send_to_daemon(msg).await {
        Ok(res) => print!("{}", res),
        Err(e) => println!("Failed to contact daemon: {}", e),
    }
}
