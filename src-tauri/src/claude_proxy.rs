use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::instance::model::{
    ActionDetails, ActionFormField, ActionFormFieldType, ActionFormSpec, ActionOption,
    ActionOptionStyle, ActionSubmission, PendingAction, PendingActionKind,
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
    raw_payload: &Value,
    submission: ActionSubmission,
) -> Result<Value> {
    match event_name {
        "PermissionRequest" => build_permission_request_response(raw_payload, submission),
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

    if tool_name == "AskUserQuestion" {
        return ask_user_question_action(action_id, message, tool_name, tool_input, payload);
    }

    let mut options = vec![ActionOption {
        id: "allow".to_string(),
        label: "Allow".to_string(),
        description: Some("Continue this Claude action once".to_string()),
        style: ActionOptionStyle::Primary,
    }];

    if let Some(suggestions) = payload
        .get("permission_suggestions")
        .or_else(|| payload.get("permissionSuggestions"))
        .and_then(Value::as_array)
    {
        for (index, suggestion) in suggestions.iter().enumerate() {
            options.push(ActionOption {
                id: format!("allow_suggestion_{index}"),
                label: permission_suggestion_label(suggestion, index),
                description: Some("Apply Claude's suggested permission update".to_string()),
                style: ActionOptionStyle::Secondary,
            });
        }
    }

    options.push(ActionOption {
        id: "deny".to_string(),
        label: "Deny".to_string(),
        description: Some("Reject this Claude action".to_string()),
        style: ActionOptionStyle::Danger,
    });

    PendingAction {
        action_id,
        title: "Permission required".to_string(),
        message,
        source_event: "PermissionRequest".to_string(),
        kind: PendingActionKind::Choice,
        options,
        input: None,
        form: None,
        details: Some(ActionDetails::ToolUse {
            tool_name,
            tool_input,
        }),
        raw_payload: payload.clone(),
    }
}

fn ask_user_question_action(
    action_id: String,
    message: String,
    tool_name: String,
    tool_input: Value,
    payload: &Value,
) -> PendingAction {
    let fields = tool_input
        .get("questions")
        .and_then(Value::as_array)
        .map(|questions| {
            questions
                .iter()
                .enumerate()
                .map(|(index, question)| ask_user_question_field(index, question))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    PendingAction {
        action_id,
        title: "Claude needs your input".to_string(),
        message,
        source_event: "PermissionRequest".to_string(),
        kind: PendingActionKind::Form,
        options: Vec::new(),
        input: None,
        form: Some(ActionFormSpec {
            submit_label: "Submit".to_string(),
            fields,
        }),
        details: Some(ActionDetails::ToolUse {
            tool_name,
            tool_input,
        }),
        raw_payload: payload.clone(),
    }
}

fn ask_user_question_field(index: usize, question: &Value) -> ActionFormField {
    let label = question
        .get("question")
        .or_else(|| question.get("header"))
        .and_then(Value::as_str)
        .unwrap_or("Question")
        .to_string();
    let multi_select = question
        .get("multiSelect")
        .or_else(|| question.get("multi_select"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let options: Vec<ActionOption> = question
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .enumerate()
                .map(|(option_index, option)| ActionOption {
                    id: option_index.to_string(),
                    label: option
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or("Option")
                        .to_string(),
                    description: option
                        .get("description")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                    style: if option_index == 0 {
                        ActionOptionStyle::Primary
                    } else {
                        ActionOptionStyle::Secondary
                    },
                })
                .collect()
        })
        .unwrap_or_default();

    ActionFormField {
        id: format!("question_{index}"),
        label,
        field_type: if options.is_empty() {
            ActionFormFieldType::Text
        } else if multi_select {
            ActionFormFieldType::MultiSelect
        } else {
            ActionFormFieldType::Select
        },
        placeholder: question
            .get("placeholder")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        required: true,
        options,
    }
}

fn permission_suggestion_label(suggestion: &Value, index: usize) -> String {
    let destination = suggestion
        .get("destination")
        .and_then(Value::as_str)
        .unwrap_or("session");
    let behavior = suggestion
        .get("behavior")
        .and_then(Value::as_str)
        .unwrap_or("allow");

    match (behavior, destination) {
        ("allow", "localSettings") => "Always allow for this project".to_string(),
        ("allow", "userSettings") => "Always allow globally".to_string(),
        ("allow", "session") => "Allow for this session".to_string(),
        ("allow", other) => format!("Always allow ({other})"),
        ("deny", other) => format!("Always deny ({other})"),
        _ => format!("Permission option {}", index + 1),
    }
}

fn build_permission_request_response(raw_payload: &Value, submission: ActionSubmission) -> Result<Value> {
    if is_ask_user_question(raw_payload) {
        return build_ask_user_question_response(raw_payload, submission);
    }

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
                "decision": {
                    "behavior": selected_id
                }
            }
        })),
        suggestion_id if suggestion_id.starts_with("allow_suggestion_") => {
            let index = suggestion_id
                .strip_prefix("allow_suggestion_")
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| anyhow::anyhow!("invalid permission suggestion id: {suggestion_id}"))?;
            let suggestion = raw_payload
                .get("permission_suggestions")
                .or_else(|| raw_payload.get("permissionSuggestions"))
                .and_then(Value::as_array)
                .and_then(|suggestions| suggestions.get(index))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("permission suggestion {index} not found"))?;

            Ok(json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "updatedPermissions": [suggestion]
                    }
                }
            }))
        }
        _ => bail!("unsupported PermissionRequest choice: {selected_id}"),
    }
}

fn is_ask_user_question(payload: &Value) -> bool {
    payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(Value::as_str)
        == Some("AskUserQuestion")
}

fn build_ask_user_question_response(raw_payload: &Value, submission: ActionSubmission) -> Result<Value> {
    let values = match submission {
        ActionSubmission::Form { values } => values,
        ActionSubmission::Choice { .. } | ActionSubmission::Input { .. } => {
            bail!("AskUserQuestion expects a form submission")
        }
    };

    let tool_input = raw_payload
        .get("tool_input")
        .or_else(|| raw_payload.get("toolInput"))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("AskUserQuestion payload is missing tool_input"))?;
    let questions = tool_input
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("AskUserQuestion tool_input is missing questions"))?;
    let mut answers = serde_json::Map::new();

    for (index, question) in questions.iter().enumerate() {
        let question_key = question
            .get("question")
            .or_else(|| question.get("header"))
            .and_then(Value::as_str)
            .unwrap_or("Question")
            .to_string();
        let field_id = format!("question_{index}");
        let raw_value = values
            .get(&field_id)
            .ok_or_else(|| anyhow::anyhow!("missing answer for {field_id}"))?;
        answers.insert(question_key, ask_user_question_answer_value(question, raw_value));
    }

    let mut updated_input = tool_input;
    if let Some(object) = updated_input.as_object_mut() {
        object.insert("answers".to_string(), Value::Object(answers));
    } else {
        bail!("AskUserQuestion tool_input must be an object");
    }

    Ok(json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": {
                "behavior": "allow",
                "updatedInput": updated_input
            }
        }
    }))
}

fn ask_user_question_answer_value(question: &Value, raw_value: &Value) -> Value {
    let selected_labels = match raw_value {
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|id| option_label_for_id(question, id))
            .collect::<Vec<_>>(),
        Value::String(id) => option_label_for_id(question, id)
            .map(|label| vec![label])
            .unwrap_or_else(|| vec![id.clone()]),
        _ => Vec::new(),
    };

    if selected_labels.len() == 1 {
        Value::String(selected_labels[0].clone())
    } else {
        Value::Array(selected_labels.into_iter().map(Value::String).collect())
    }
}

fn option_label_for_id(question: &Value, id: &str) -> Option<String> {
    let index = id.parse::<usize>().ok()?;
    question
        .get("options")
        .and_then(Value::as_array)
        .and_then(|options| options.get(index))
        .and_then(|option| option.get("label"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{build_hook_response, pending_action_from_hook};
    use crate::instance::model::{ActionFormFieldType, ActionSubmission, PendingActionKind};
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
            "message": "Claude needs permission to run Bash",
            "permission_suggestions": [
                {
                    "type": "rule",
                    "rule": "Bash(git status)",
                    "behavior": "allow",
                    "destination": "localSettings"
                }
            ]
        });

        let action = pending_action_from_hook("PermissionRequest", &payload)
            .unwrap()
            .expect("expected blocking action");

        assert_eq!(action.action_id, "tool-123");
        assert_eq!(action.kind, PendingActionKind::Choice);
        assert_eq!(action.source_event, "PermissionRequest");
        assert_eq!(action.options.len(), 3);
        assert_eq!(action.options[0].id, "allow");
        assert_eq!(action.options[1].id, "allow_suggestion_0");
        assert_eq!(action.options[2].id, "deny");
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
                    "decision": {
                        "behavior": "allow"
                    }
                }
            })
        );
    }

    #[test]
    fn permission_request_suggestion_submission_builds_updated_permissions() {
        let payload = json!({
            "permission_suggestions": [
                {
                    "type": "rule",
                    "rule": "Bash(git status)",
                    "behavior": "allow",
                    "destination": "localSettings"
                }
            ]
        });
        let response = build_hook_response(
            "PermissionRequest",
            &payload,
            ActionSubmission::Choice {
                selected_id: "allow_suggestion_0".to_string(),
            },
        )
        .unwrap();

        assert_eq!(
            response,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "updatedPermissions": [
                            {
                                "type": "rule",
                                "rule": "Bash(git status)",
                                "behavior": "allow",
                                "destination": "localSettings"
                            }
                        ]
                    }
                }
            })
        );
    }

    #[test]
    fn ask_user_question_maps_to_form_options() {
        let payload = json!({
            "session_id": "session-1",
            "tool_name": "AskUserQuestion",
            "tool_input": {
                "questions": [
                    {
                        "header": "Hook 场景",
                        "question": "选择要测试的场景",
                        "multiSelect": false,
                        "options": [
                            {
                                "label": "权限命令拦截",
                                "description": "测试 hooks 是否能拦截需要权限确认的命令"
                            },
                            {
                                "label": "跳过"
                            }
                        ]
                    }
                ]
            },
            "tool_use_id": "tool-question",
            "message": "Allow Claude Code to use AskUserQuestion?"
        });

        let action = pending_action_from_hook("PermissionRequest", &payload)
            .unwrap()
            .expect("expected blocking action");
        let form = action.form.expect("AskUserQuestion should render as a form");

        assert_eq!(action.kind, PendingActionKind::Form);
        assert_eq!(form.fields.len(), 1);
        assert_eq!(form.fields[0].field_type, ActionFormFieldType::Select);
        assert_eq!(form.fields[0].options[0].label, "权限命令拦截");
        assert_eq!(form.fields[0].options[1].label, "跳过");
    }

    #[test]
    fn ask_user_question_submission_builds_updated_input_answers() {
        let payload = json!({
            "tool_name": "AskUserQuestion",
            "tool_input": {
                "questions": [
                    {
                        "question": "选择要测试的场景",
                        "multiSelect": false,
                        "options": [
                            { "label": "权限命令拦截" },
                            { "label": "跳过" }
                        ]
                    }
                ]
            }
        });
        let mut values = serde_json::Map::new();
        values.insert("question_0".to_string(), json!("0"));

        let response = build_hook_response(
            "PermissionRequest",
            &payload,
            ActionSubmission::Form { values },
        )
        .unwrap();

        assert_eq!(
            response,
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "updatedInput": {
                            "questions": [
                                {
                                    "question": "选择要测试的场景",
                                    "multiSelect": false,
                                    "options": [
                                        { "label": "权限命令拦截" },
                                        { "label": "跳过" }
                                    ]
                                }
                            ],
                            "answers": {
                                "选择要测试的场景": "权限命令拦截"
                            }
                        }
                    }
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
