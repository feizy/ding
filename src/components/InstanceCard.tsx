import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Instance, ActionDecision } from '../types/instance';
import { StatusDot } from './StatusDot';
import { ActionPanel } from './ActionPanel';
import { LogViewer } from './LogViewer';
import { useInstanceStore } from '../stores/instanceStore';

interface InstanceCardProps {
  instance: Instance;
}

const statusLabel: Record<string, string> = {
  idle:            'Waiting…',
  thinking:        'Thinking…',
  running:         'Running…',
  action_required: 'Action needed',
  error:           'Error',
  finished:        'Completed',
};

export const InstanceCard = ({ instance }: InstanceCardProps) => {
  const [showLogs, setShowLogs] = useState(false);
  const clearPendingAction  = useInstanceStore(s => s.clearPendingAction);
  const removeInstance       = useInstanceStore(s => s.removeInstance);

  const isAction   = instance.status === 'action_required';
  const isFinished = instance.status === 'finished';
  const isError    = instance.status === 'error';
  const lastLog    = instance.recent_logs[instance.recent_logs.length - 1];

  const cardClass = [
    'card',
    isAction   ? 'card--action-required' : '',
    isFinished ? 'card--finished'        : '',
    isError    ? 'card--error'           : '',
  ].filter(Boolean).join(' ');

  const handleDecision = (decision: ActionDecision) => {
    invoke('send_decision', { instanceId: instance.id, decision })
      .catch(console.error);
    clearPendingAction(instance.id);
  };

  const handleKill = (e: React.MouseEvent) => {
    e.stopPropagation();
    invoke('kill_instance', { instanceId: instance.id })
      .catch(console.error);
    removeInstance(instance.id);
  };

  const elapsedSecs = Math.floor(
    (Date.now() - new Date(instance.created_at).getTime()) / 1000
  );
  const elapsedLabel = elapsedSecs < 60
    ? `${elapsedSecs}s`
    : `${Math.floor(elapsedSecs / 60)}m`;

  return (
    <div
      className={cardClass}
      onClick={() => !isAction && setShowLogs(v => !v)}
    >
      <div className="card__header">
        <StatusDot status={instance.status} />
        <div className="card__info">
          <div className="card__name">
            {instance.name}
            <span className="card__adapter">{instance.adapter_label}</span>
            <span className="card__id">{instance.id}</span>
          </div>
          <div className="card__status-text">
            {isAction && instance.pending_action
              ? instance.pending_action.message
              : lastLog
              ? lastLog.text
              : statusLabel[instance.status] || ''}
          </div>
        </div>

        {/* Right side: cost + elapsed + kill */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0 }}>
          {instance.cost_usd != null && (
            <span style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>
              ${instance.cost_usd.toFixed(3)}
            </span>
          )}
          <span style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>
            {elapsedLabel}
          </span>
          <button
            id={`kill-${instance.id}`}
            className="card__kill"
            onClick={handleKill}
            title="Kill instance"
          >
            ✕
          </button>
        </div>
      </div>

      {/* Action area */}
      {isAction && instance.pending_action && (
        <ActionPanel action={instance.pending_action} onDecision={handleDecision} />
      )}

      {/* Expandable log viewer */}
      {showLogs && !isAction && <LogViewer logs={instance.recent_logs} />}
    </div>
  );
};
