import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ActionSubmission, Instance } from '../types/instance';
import { ActionPanel } from './ActionPanel';
import { LogViewer } from './LogViewer';
import { StatusDot } from './StatusDot';
import { useInstanceStore } from '../stores/instanceStore';

interface InstanceCardProps {
  instance: Instance;
}

const statusLabel: Record<string, string> = {
  idle: 'Waiting',
  thinking: 'Thinking',
  tool_calling: 'Using tool',
  running: 'Running',
  action_required: 'Action needed',
  error: 'Error',
  finished: 'Completed',
};

export const InstanceCard = ({ instance }: InstanceCardProps) => {
  const [showLogs, setShowLogs] = useState(false);
  const removeInstance = useInstanceStore((state) => state.removeInstance);

  const isAction = instance.status === 'action_required';
  const isFinished = instance.status === 'finished';
  const isError = instance.status === 'error';
  const lastLog = instance.recent_logs[instance.recent_logs.length - 1];

  const cardClass = [
    'card',
    isAction ? 'card--action-required' : '',
    isFinished ? 'card--finished' : '',
    isError ? 'card--error' : '',
  ]
    .filter(Boolean)
    .join(' ');

  const handleSubmit = (submission: ActionSubmission) => {
    invoke('submit_action', { instanceId: instance.id, submission }).catch(console.error);
  };

  const handleKill = (event: React.MouseEvent) => {
    event.stopPropagation();
    invoke('kill_instance', { instanceId: instance.id }).catch(console.error);
    removeInstance(instance.id);
  };

  const elapsedSecs = Math.floor((Date.now() - new Date(instance.created_at).getTime()) / 1000);
  const elapsedLabel = elapsedSecs < 60 ? `${elapsedSecs}s` : `${Math.floor(elapsedSecs / 60)}m`;

  const statusText =
    isAction && instance.pending_action
      ? instance.pending_action.message
      : instance.status === 'tool_calling' && instance.current_tool_name
      ? `Using ${instance.current_tool_name}`
      : lastLog
      ? lastLog.text
      : statusLabel[instance.status] || '';

  return (
    <div className={cardClass} onClick={() => !isAction && setShowLogs((value) => !value)}>
      <div className="card__header">
        <StatusDot status={instance.status} />
        <div className="card__info">
          <div className="card__name">
            {instance.name}
            <span className="card__adapter">{instance.adapter_label}</span>
            <span className="card__id">{instance.id}</span>
          </div>
          <div className="card__status-text">{statusText}</div>
        </div>

        <div className="card__meta">
          {instance.cost_usd != null && (
            <span className="card__cost">${instance.cost_usd.toFixed(3)}</span>
          )}
          <span className="card__elapsed">{elapsedLabel}</span>
          <button
            id={`kill-${instance.id}`}
            className="card__kill"
            onClick={handleKill}
            data-no-drag="true"
            title="Kill instance"
          >
            ×
          </button>
        </div>
      </div>

      {isAction && instance.pending_action && (
        <ActionPanel action={instance.pending_action} onSubmit={handleSubmit} />
      )}

      {showLogs && !isAction && <LogViewer logs={instance.recent_logs} />}
    </div>
  );
};
