import React from 'react';
import type { PendingAction, ActionDecision } from '../types/instance';

interface ActionPanelProps {
  action: PendingAction;
  onDecision: (decision: ActionDecision) => void;
}

const decisionConfig: Record<ActionDecision, { label: string; icon: string; cls: string }> = {
  approve: { label: 'Approve', icon: '✓', cls: 'btn--approve' },
  approve_for_session: { label: 'Always Allow', icon: '✦', cls: 'btn--session' },
  deny: { label: 'Deny', icon: '✕', cls: 'btn--deny' },
  abort: { label: 'Abort', icon: '■', cls: 'btn--abort' },
};

export const ActionPanel: React.FC<ActionPanelProps> = ({ action, onDecision }) => {
  const details = action.details;

  const renderPreview = () => {
    if (details.type === 'tool_use') {
      const input = details.tool_input as Record<string, unknown>;
      const cmd = input?.command as string | undefined;
      return (
        <div className="action-area__preview">
          <div className="action-area__label">{details.tool_name}</div>
          {cmd && <div className="action-area__command">{cmd}</div>}
          {!cmd && (
            <div className="action-area__command" style={{ color: 'var(--text-secondary)' }}>
              {JSON.stringify(input, null, 2).slice(0, 120)}
            </div>
          )}
        </div>
      );
    }

    if (details.type === 'command') {
      return (
        <div className="action-area__preview">
          <div className="action-area__label">Command</div>
          <div className="action-area__command">{details.command?.join(' ')}</div>
          {details.cwd && <div className="action-area__cwd">cwd: {details.cwd}</div>}
          {details.reason && (
            <div style={{ fontSize: 10, color: 'var(--text-tertiary)', marginTop: 4 }}>
              {details.reason}
            </div>
          )}
        </div>
      );
    }

    if (details.type === 'file_diff') {
      return (
        <div className="action-area__preview">
          <div className="action-area__label">File Changes</div>
          {details.files?.map((f, i) => (
            <div key={i} style={{ fontSize: 11, color: 'var(--text-secondary)', marginTop: 2 }}>
              📄 {f.path}
              <span style={{ color: 'var(--color-finished)', marginLeft: 6 }}>+{f.additions}</span>
              <span style={{ color: 'var(--color-error)', marginLeft: 4 }}>-{f.deletions}</span>
            </div>
          ))}
        </div>
      );
    }

    return null;
  };

  return (
    <div className="action-area">
      {renderPreview()}
      <div className="action-area__buttons">
        {action.available_decisions.map((d) => {
          const cfg = decisionConfig[d];
          return (
            <button
              key={d}
              className={`btn ${cfg.cls}`}
              onClick={(e) => {
                e.stopPropagation();
                onDecision(d);
              }}
            >
              {cfg.icon} {cfg.label}
            </button>
          );
        })}
      </div>
    </div>
  );
};
