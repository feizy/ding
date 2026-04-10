use std::env;
use std::process::{Command, exit};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Fallback: If no DING_HOOK_PIPE is provided, we just execute blindly (passthrough).
    let pipe_name = match env::var("DING_HOOK_PIPE") {
        Ok(val) => val,
        Err(_) => {
            if args.len() > 1 {
                return exec_passthrough(&args[1..]);
            }
            exit(0);
        }
    };

    // We have a pipe, so we need to ask for approval
    if args.len() < 2 {
        exit(0); // Nothing to do
    }
    let target_cmd = &args[1..];
    let cmd_str = target_cmd.join(" ");

    // Connect to the UI via Named Pipe
    let mut client = match ClientOptions::new().open(&pipe_name) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ding-hook: Failed to connect to {} - {}", pipe_name, e);
            // Default to deny if we can't connect, to be safe.
            eprintln!("ding-hook: Denying execution by default.");
            exit(1);
        }
    };

    // Create ActionRequired payload
    let payload = serde_json::json!({
        "type": "action_required",
        "action": {
            "action_id": format!("{}", chrono::Utc::now().timestamp()),
            "message": format!("Requesting execution: {}", cmd_str),
            "available_decisions": ["Approve", "Deny", "Abort"],
            "details": {
                "command": target_cmd,
                "cwd": env::current_dir().unwrap_or_default().display().to_string(),
                "reason": "Agent hook interception"
            }
        }
    });

    let mut msg = payload.to_string();
    msg.push('\n');

    if let Err(e) = client.write_all(msg.as_bytes()).await {
        eprintln!("ding-hook: Failed to write to pipe: {}", e);
        exit(1);
    }

    // Wait for the decision
    let mut buf = vec![0u8; 1024];
    let n = match client.read(&mut buf).await {
        Ok(n) => n,
        Err(e) => {
            eprintln!("ding-hook: Failed to read from pipe: {}", e);
            exit(1);
        }
    };

    let decision_str = String::from_utf8_lossy(&buf[..n]).trim().to_lowercase();

    match decision_str.as_str() {
        "approve" | "approveforsession" => {
            exec_passthrough(target_cmd);
        }
        "deny" => {
            println!("ding-hook: Error - Execution denied by user.");
            exit(1);
        }
        "abort" => {
            println!("ding-hook: Error - Session aborted by user.");
            // Return exit code 130 (SIGINT) to signal abort
            exit(130);
        }
        _ => {
            eprintln!("ding-hook: Received unknown decision: {}", decision_str);
            exit(1);
        }
    }
}

// Executes the target command transparently
fn exec_passthrough(cmd_args: &[String]) {
    if cmd_args.is_empty() {
        exit(0);
    }

    let result = Command::new(&cmd_args[0])
        .args(&cmd_args[1..])
        .status();

    match result {
        Ok(status) => {
            exit(status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("ding-hook failed to execute {}: {}", cmd_args[0], e);
            exit(1);
        }
    }
}
