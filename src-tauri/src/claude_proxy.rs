use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::instance::model::{
    ActionDetails, ActionOption, ActionOptionStyle, ActionSubmission, PendingAction,
    PendingActionKind,
};

pub fn pending_action_from_hook(
    event_name: &str,
    payload: &Value,
) -> Result<Option<PendingAction>> {
    match event_name {
        "PermissionRequest" => Ok(Some(permission_request_action(payload))),
        _ => Ok(None),
    }
}

pub fn build_hook_response(
    event_name: &str,
    _raw_payload: &Value,
    submission: ActionSubmission,
) -> Result<Value> {
    match event_name {
        "PermissionRequest" => build_permission_request_response(submission),
        _ => bail!("unsupported blocking hook event: {event_name}"),
    }
}

fn permission_request_action(payload: &Value) -> PendingAction {
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(Value::as_str)
        .unwrap_or("tool")
        .to_string();

    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .cloned()
        .unwrap_or(Value::Null);

    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("Allow Claude Code to use {tool_name}?"));

    let action_id = payload
        .get("tool_use_id")
        .or_else(|| payload.get("toolUseId"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("permission-{}", uuid::Uuid::new_v4()));

    PendingAction {
        action_id,
        title: "Permission required".to_string(),
        message,
        source_event: "PermissionRequest".to_string(),
        kind: PendingActionKind::Choice,
        options: vec![
            ActionOption {
                id: "allow".to_string(),
                label: "Allow".to_string(),
                description: Some("Continue this Claude action".to_string()),
                style: ActionOptionStyle::Primary,
            },
            ActionOption {
                id: "deny".to_string(),
                label: "Deny".to_string(),
                description: Some("Reject this Claude action".to_string()),
                style: ActionOptionStyle::Danger,
            },
        ],
        input: None,
        form: None,
        details: Some(ActionDetails::ToolUse {
            tool_name,
            tool_input,
        }),
        raw_payload: payload.clone(),
    }
}

fn build_permission_request_response(submission: ActionSubmission) -> Result<Value> {
    let selected_id = match submission {
        ActionSubmission::Choice { selected_id } => selected_id,
        ActionSubmission::Input { .. } | ActionSubmission::Form { .. } => {
            bail!("PermissionRequest expects a choice submission")
        }
    };

    match selected_id.as_str() {
        "allow" | "deny" => Ok(json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "permissionDecision": selected_id,
                "permissionDecisionReason": "Resolved in ding"
            }
        })),
        _ => bail!("unsupported PermissionRequest choice: {selected_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{build_hook_response, pending_action_from_hook};
    use crate::instance::model::{ActionSubmission, PendingActionKind};
    use serde_json::json;

    #[test]
    fn permission_request_maps_to_choice_action() {
        let payload = json!({
            "session_id": "session-1",
            "tool_name": "Bash",
            "tool_input": {
                "command": "git status"
            },
            "tool_use_id": "tool-123",
            "message": "Claude needs permission to run Bash"
        });

        let action = pending_action_from_hook("PermissionRequest", &payload)
            .unwrap()
            .expect("expected blocking action");

        assert_eq!(action.action_id, "tool-123");
        assert_eq!(action.kind, PendingActionKind::Choice);
        assert_eq!(action.source_event, "PermissionRequest");
        assert_eq!(action.options.len(), 2);
        assert_eq!(action.options[0].id, "allow");
        assert_eq!(action.options[1].id, "deny");
    }

    #[test]
    fn permission_request_submission_builds_hook_specific_output() {
        let response = build_hook_response(
            "PermissionRequest",
            &json!({}),
            ActionSubmission::Choice {
                selected_id: "allow".to_string(),
            },
        )
        .unwrap();

        assert_eq!(
            response,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Resolved in ding"
                }
            })
        );
    }

    #[test]
    fn permission_request_rejects_non_choice_submission() {
        let error = build_hook_response(
            "PermissionRequest",
            &json!({}),
            ActionSubmission::Input {
                value: "nope".to_string(),
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("expects a choice submission"));
    }
}
