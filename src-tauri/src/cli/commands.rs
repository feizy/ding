/// CLI command handlers
use super::Commands;
use crate::ipc::{self, IpcMessage};

pub async fn handle_command(cmd: Commands) {
    match cmd {
        Commands::Claude { args } => {
            if let Err(err) = ensure_daemon_running().await {
                eprintln!("Failed to start ding daemon: {err}");
                std::process::exit(1);
            }

            let exe_path = match std::env::current_exe() {
                Ok(path) => path,
                Err(err) => {
                    eprintln!("Failed to resolve current executable: {err}");
                    std::process::exit(1);
                }
            };

            if let Err(err) = crate::claude_hooks::ensure_user_hooks_installed(&exe_path) {
                eprintln!("Failed to install Claude hooks: {err}");
                std::process::exit(1);
            }

            match crate::claude_hooks::launch_native_claude(&args) {
                Ok(status) => std::process::exit(status.code().unwrap_or(1)),
                Err(err) => {
                    eprintln!("Failed to launch Claude: {err}");
                    std::process::exit(1);
                }
            }
        }
        Commands::HookRelay { event_name } => {
            if let Err(err) = crate::claude_hooks::relay_hook_event(&event_name).await {
                eprintln!("Failed to relay Claude hook event: {err}");
                std::process::exit(1);
            }
        }
        other => {
            if let Err(err) = ensure_daemon_running().await {
                eprintln!("Failed to start ding daemon: {err}");
                std::process::exit(1);
            }

            let msg = match other {
                Commands::Codex { prompt, name, model, approval_mode } => {
                    IpcMessage::Codex { prompt, name, model, approval_mode }
                }
                Commands::Run { program, args, name } => {
                    IpcMessage::Run { program, args, name }
                }
                Commands::List => IpcMessage::List,
                Commands::Kill { id } => IpcMessage::Kill { id },
                Commands::KillAll => IpcMessage::KillAll,
                Commands::Claude { .. } | Commands::HookRelay { .. } => unreachable!(),
            };

            match ipc::send_to_daemon(msg).await {
                Ok(res) => print!("{}", res),
                Err(e) => println!("Failed to contact daemon: {}", e),
            }
        }
    }
}

async fn ensure_daemon_running() -> Result<(), String> {
    if ipc::is_daemon_running() {
        return Ok(());
    }

    println!("Ding daemon not running. Starting in background...");
    let exe_path = std::env::current_exe().map_err(|err| err.to_string())?;
    std::process::Command::new(exe_path)
        .spawn()
        .map_err(|err| err.to_string())?;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    Ok(())
}
