export type DingStatus =
  | 'action_required'
  | 'error'
  | 'thinking'
  | 'tool_calling'
  | 'running'
  | 'idle'
  | 'finished';

export type AdapterType = 'claude_code' | 'codex' | 'generic';

export type ActionDecision = 'approve' | 'approve_for_session' | 'deny' | 'abort';
export type PendingActionKind = 'choice' | 'input' | 'form';
export type ActionOptionStyle = 'primary' | 'secondary' | 'danger';

export interface ActionOption {
  id: string;
  label: string;
  description: string | null;
  style: ActionOptionStyle;
}

export interface ActionInputSpec {
  placeholder: string | null;
  submit_label: string;
  multiline: boolean;
}

export type ActionFormFieldType = 'text' | 'multiline' | 'select' | 'multi_select';

export interface ActionFormField {
  id: string;
  label: string;
  field_type: ActionFormFieldType;
  placeholder: string | null;
  required: boolean;
  options: ActionOption[];
}

export interface ActionFormSpec {
  submit_label: string;
  fields: ActionFormField[];
}

export interface ActionDetails {
  type: 'command' | 'file_diff' | 'tool_use';
  command?: string[];
  cwd?: string;
  reason?: string | null;
  files?: FileDiffEntry[];
  tool_name?: string;
  tool_input?: Record<string, unknown>;
}

export interface FileDiffEntry {
  path: string;
  additions: number;
  deletions: number;
}

export interface PendingAction {
  action_id: string;
  title: string;
  message: string;
  source_event: string;
  kind: PendingActionKind;
  options: ActionOption[];
  input: ActionInputSpec | null;
  form: ActionFormSpec | null;
  details: ActionDetails | null;
}

export type ActionSubmission =
  | { kind: 'choice'; selected_id: string }
  | { kind: 'input'; value: string }
  | { kind: 'form'; values: Record<string, string | string[]> };

export interface LogLine {
  timestamp: string;
  text: string;
  level: 'info' | 'tool' | 'error' | 'system';
}

export interface Instance {
  id: string;
  name: string;
  adapter_type: AdapterType;
  adapter_label: string;
  status: DingStatus;
  current_tool_name: string | null;
  created_at: string;
  last_event_at: string;
  pending_action: PendingAction | null;
  recent_logs: LogLine[];
  exit_code: number | null;
  cost_usd: number | null;
}
