// ding status type definitions — mirrors Rust backend

export type DingStatus =
  | 'action_required'
  | 'error'
  | 'thinking'
  | 'running'
  | 'idle'
  | 'finished';

export type AdapterType = 'claude_code' | 'codex' | 'generic';

export type ActionDecision = 'approve' | 'approve_for_session' | 'deny' | 'abort';

export interface ActionDetails {
  type: 'command' | 'file_diff' | 'tool_use';
  command?: string[];
  cwd?: string;
  reason?: string;
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
  message: string;
  available_decisions: ActionDecision[];
  details: ActionDetails;
}

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
  created_at: string;
  last_event_at: string;
  pending_action: PendingAction | null;
  recent_logs: LogLine[];
  exit_code: number | null;
  cost_usd: number | null;
}

export interface InstanceListResponse {
  instances: Instance[];
  total: number;
  action_required_count: number;
  primary_status: DingStatus;
}
