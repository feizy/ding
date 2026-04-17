/// CLI command handlers
use super::Commands;
use crate::ipc::{self, IpcMessage};
use std::process::{Child, Command, Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
                Commands::Decide { instance_id, decision } => {
                    IpcMessage::SendDecision { instance_id, decision }
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
    let mut command = Command::new(exe_path);
    configure_background_daemon_command(&mut command);
    let mut child = command
        .spawn()
        .map_err(|err| err.to_string())?;

    let started = wait_for_daemon_ready(&mut child).await?;
    if started {
        Ok(())
    } else {
        Err("daemon did not become reachable in time".to_string())
    }
}

async fn wait_for_daemon_ready(child: &mut Child) -> Result<bool, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);

    loop {
        if ipc::is_daemon_running() {
            return Ok(true);
        }

        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            return Err(format!("daemon exited early with status {}", status));
        }

        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }

        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

fn configure_background_daemon_command(command: &mut Command) {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}
