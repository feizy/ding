use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const CLAUDE_MONITOR_ENV: &str = "DING_MONITOR_CLAUDE";
const MANAGED_HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Notification",
    "Stop",
    "SubagentStop",
    "SessionEnd",
];

const LEGACY_MANAGED_HOOK_EVENTS: &[&str] = &[
    "PermissionDenied",
    "PostToolUseFailure",
    "StopFailure",
];

const TOOL_MATCHED_EVENTS: &[&str] = &["PreToolUse", "PostToolUse"];

pub fn ensure_user_hooks_installed(main_exe_path: &Path) -> Result<PathBuf> {
    let settings_path = user_settings_path()?;
    let mut settings = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .with_context(|| format!("failed to read {}", settings_path.display()))?;
        serde_json::from_str::<Value>(&content)
            .with_context(|| format!("failed to parse {}", settings_path.display()))?
    } else {
        json!({})
    };

    if merge_managed_hooks(&mut settings, main_exe_path) {
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let formatted = serde_json::to_string_pretty(&settings)?;
        fs::write(&settings_path, format!("{formatted}\n"))
            .with_context(|| format!("failed to write {}", settings_path.display()))?;
    }

    Ok(settings_path)
}

pub fn merge_managed_hooks(settings: &mut Value, main_exe_path: &Path) -> bool {
    if !settings.is_object() {
        *settings = json!({});
    }

    let settings_obj = settings.as_object_mut().unwrap();
    let hooks_value = settings_obj
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if !hooks_value.is_object() {
        *hooks_value = Value::Object(Map::new());
    }

    let hooks_obj = hooks_value.as_object_mut().unwrap();
    let mut changed = false;

    for legacy_event in LEGACY_MANAGED_HOOK_EVENTS {
        if let Some(groups_value) = hooks_obj.get_mut(*legacy_event) {
            if let Some(groups) = groups_value.as_array_mut() {
                let before = groups.clone();
                for group in groups.iter_mut() {
                    if let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                        hooks.retain(|hook| !is_managed_hook(hook, legacy_event));
                    }
                }
                groups.retain(|group| {
                    group
                        .get("hooks")
                        .and_then(Value::as_array)
                        .map(|hooks| !hooks.is_empty())
                        .unwrap_or(true)
                });
                if *groups != before {
                    changed = true;
                }
            }
        }
        if hooks_obj
            .get(*legacy_event)
            .and_then(Value::as_array)
            .map(|groups| groups.is_empty())
            .unwrap_or(false)
        {
            hooks_obj.remove(*legacy_event);
            changed = true;
        }
    }

    for event_name in MANAGED_HOOK_EVENTS {
        let groups_value = hooks_obj
            .entry((*event_name).to_string())
            .or_insert_with(|| Value::Array(Vec::new()));

        if !groups_value.is_array() {
            *groups_value = Value::Array(Vec::new());
            changed = true;
        }

        let groups = groups_value.as_array_mut().unwrap();
        let before = groups.clone();

        for group in groups.iter_mut() {
            if let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                hooks.retain(|hook| !is_managed_hook(hook, event_name));
            }
        }

        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .map(|hooks| !hooks.is_empty())
                .unwrap_or(true)
        });

        groups.push(managed_hook_group(main_exe_path, event_name));

        if *groups != before {
            changed = true;
        }
    }

    changed
}

pub async fn relay_hook_event(event_name: &str) -> Result<()> {
    let mut stdin = tokio::io::stdin();
    let mut payload = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut stdin, &mut payload).await?;

    if !crate::ipc::is_daemon_running() {
        start_daemon_in_background()?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let parsed = if payload.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&payload).unwrap_or_else(|_| {
            json!({
                "raw_stdin": payload
            })
        })
    };

    let response = crate::ipc::send_to_daemon(crate::ipc::IpcMessage::ClaudeHookEvent {
        event_name: event_name.to_string(),
        payload: parsed,
    })
    .await?;

    if !response.trim().is_empty() {
        println!("{response}");
    }

    Ok(())
}

pub fn launch_native_claude(args: &[String]) -> Result<ExitStatus> {
    let status = Command::new("claude")
        .args(args)
        .env(CLAUDE_MONITOR_ENV, "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("failed to launch native claude process")?;

    Ok(status)
}

fn user_settings_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| anyhow!("HOME/USERPROFILE is not set"))?;

    Ok(PathBuf::from(home).join(".claude").join("settings.json"))
}

fn managed_hook_group(main_exe_path: &Path, event_name: &str) -> Value {
    let mut group = json!({
        "hooks": [
            {
                "type": "command",
                "shell": "powershell",
                "command": build_managed_hook_command(main_exe_path, event_name)
            }
        ]
    });

    if TOOL_MATCHED_EVENTS.contains(&event_name) {
        group["matcher"] = Value::String("*".to_string());
    }

    group
}

fn build_managed_hook_command(main_exe_path: &Path, event_name: &str) -> String {
    let escaped = main_exe_path.display().to_string().replace('\'', "''");
    format!(
        "if ($env:{CLAUDE_MONITOR_ENV} -eq '1') {{ & '{escaped}' hook-relay {event_name} }}"
    )
}

fn is_managed_hook(hook: &Value, event_name: &str) -> bool {
    hook.get("type").and_then(Value::as_str) == Some("command")
        && hook
            .get("command")
            .and_then(Value::as_str)
            .map(|command| command.contains(&format!("hook-relay {event_name}")))
            .unwrap_or(false)
}

fn start_daemon_in_background() -> Result<()> {
    let exe_path = std::env::current_exe().context("failed to resolve current executable")?;
    let mut command = Command::new(exe_path);
    configure_background_daemon_command(&mut command);
    command
        .spawn()
        .context("failed to start ding daemon in background")?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::merge_managed_hooks;
    use serde_json::json;

    #[test]
    fn merge_settings_adds_managed_hooks_without_removing_existing_entries() {
        let mut settings = json!({
            "theme": "dark",
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "echo existing"
                            }
                        ]
                    }
                ]
            }
        });

        let changed = merge_managed_hooks(
            &mut settings,
            std::path::Path::new(r"C:\tools\ding.exe"),
        );

        assert!(changed);
        assert_eq!(settings["theme"], "dark");
        assert_eq!(settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "echo existing");

        let pre_tool_groups = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert!(pre_tool_groups.iter().any(|group| {
            group["hooks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|hook| hook["command"].as_str().unwrap().contains("hook-relay PreToolUse"))
        }));

        let session_start_groups = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_start_groups.len(), 1);
        assert!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .contains("$env:DING_MONITOR_CLAUDE -eq '1'")
        );
    }

    #[test]
    fn merge_settings_is_idempotent_for_managed_hooks() {
        let mut settings = json!({});

        let changed_once = merge_managed_hooks(
            &mut settings,
            std::path::Path::new(r"C:\tools\ding.exe"),
        );
        let snapshot = settings.clone();
        let changed_twice = merge_managed_hooks(
            &mut settings,
            std::path::Path::new(r"C:\tools\ding.exe"),
        );

        assert!(changed_once);
        assert!(!changed_twice);
        assert_eq!(settings, snapshot);
    }

    #[test]
    fn merge_settings_removes_legacy_managed_hook_entries() {
        let mut settings = json!({
            "hooks": {
                "PermissionDenied": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "& 'C:\\tools\\ding.exe' hook-relay PermissionDenied"
                            }
                        ]
                    }
                ]
            }
        });

        let changed = merge_managed_hooks(&mut settings, std::path::Path::new(r"C:\tools\ding.exe"));

        assert!(changed);
        assert!(settings["hooks"].get("PermissionDenied").is_none());
        assert!(settings["hooks"].get("PermissionRequest").is_some());
    }
}
