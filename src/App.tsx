import { useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useInstanceStore } from './stores/instanceStore';
import { StatusDot } from './components/StatusDot';
import { InstanceCard } from './components/InstanceCard';
import type { Instance } from './types/instance';

const statusLabel: Record<string, string> = {
  idle: 'All idle',
  thinking: 'Thinking…',
  running: 'Running…',
  action_required: 'Action needed!',
  error: 'Error detected',
  finished: 'All done',
};

function App() {
  const {
    instances,
    expanded,
    toggleExpanded,
    primaryStatus,
    actionRequiredCount,
    setInstances,
    upsertInstance,
    updateInstanceStatus,
    updatePendingAction,
    appendLog,
    updateCost,
    removeInstance,
  } = useInstanceStore();

  const refreshInstances = useCallback(() => {
    invoke<Instance[]>('get_instances')
      .then((data) => setInstances(data))
      .catch(() => {}); // daemon may not be ready yet
  }, [setInstances]);

  useEffect(() => {
    // Initial load
    refreshInstances();

    // Poll every 5 seconds as fallback for missed events
    const pollId = setInterval(refreshInstances, 5000);

    // Real-time event listener
    const unlisten = listen('ding-event', (event: any) => {
      const payload = event.payload;
      switch (payload.type) {
        case 'instance_created':
          upsertInstance(payload.instance);
          break;
        case 'status_changed':
          updateInstanceStatus(payload.instance_id, payload.status);
          break;
        case 'action_required':
          updatePendingAction(payload.instance_id, payload.action);
          break;
        case 'log_appended':
          appendLog(payload.instance_id, payload.log);
          break;
        case 'cost_updated':
          updateCost(payload.instance_id, payload.cost_usd);
          break;
        case 'instance_removed':
          removeInstance(payload.instance_id);
          break;
      }
    });

    return () => {
      clearInterval(pollId);
      unlisten.then((f) => f());
    };
  }, [refreshInstances, upsertInstance, updateInstanceStatus, updatePendingAction, appendLog, updateCost, removeInstance]);

  const totalCost = instances.reduce((sum, i) => sum + (i.cost_usd ?? 0), 0);
  const primaryInstance = instances.length > 0 ? instances[0] : null;

  // Sort: action_required first, then by recency
  const sortedInstances = [...instances].sort((a, b) => {
    const priority: Record<string, number> = {
      action_required: 0, error: 1, thinking: 2, running: 3, idle: 4, finished: 5,
    };
    const pa = priority[a.status] ?? 99;
    const pb = priority[b.status] ?? 99;
    if (pa !== pb) return pa - pb;
    return new Date(b.last_event_at).getTime() - new Date(a.last_event_at).getTime();
  });

  const widgetClass = [
    'widget',
    primaryStatus === 'action_required' ? 'widget--action-required' : '',
    primaryStatus === 'error' ? 'widget--error' : '',
    expanded ? 'widget--expanded' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={widgetClass}>
      {/* ─── Capsule (always visible) ─── */}
      <div className="capsule" onClick={toggleExpanded} data-tauri-drag-region>
        <StatusDot status={primaryStatus} />

        <div className="capsule__info">
          <div className="capsule__name">
            {primaryInstance ? primaryInstance.name : 'ding'}
            {totalCost > 0 && (
              <span style={{ fontSize: 9, color: 'var(--text-tertiary)', fontWeight: 400, marginLeft: 6 }}>
                ${totalCost.toFixed(3)}
              </span>
            )}
          </div>
          <div className="capsule__subtitle">
            {actionRequiredCount > 0
              ? `⚠ ${actionRequiredCount} action${actionRequiredCount > 1 ? 's' : ''} needed`
              : primaryInstance
              ? `${primaryInstance.adapter_label} · ${statusLabel[primaryStatus] || ''}`
              : 'No active agents'}
          </div>
        </div>

        <div className="capsule__right">
          {/* Multi-instance status dots */}
          {instances.length > 1 && (
            <div className="capsule__dots">
              {sortedInstances.slice(0, 5).map((inst) => (
                <StatusDot key={inst.id} status={inst.status} size="small" />
              ))}
              {instances.length > 5 && (
                <span style={{ fontSize: 9, color: 'var(--text-tertiary)' }}>+{instances.length - 5}</span>
              )}
            </div>
          )}
          <span className={`capsule__count ${actionRequiredCount > 0 ? 'capsule__count--urgent' : ''}`}>
            {actionRequiredCount > 0
              ? `⚠ ${actionRequiredCount}`
              : instances.length > 0
              ? `${instances.length}`
              : '—'}
          </span>
          <span className={`capsule__chevron ${expanded ? 'capsule__chevron--up' : ''}`}>▾</span>
        </div>
      </div>

      {/* ─── Expanded Panel ─── */}
      {expanded && (
        <div className="panel">
          {sortedInstances.length === 0 ? (
            <div className="empty">
              <div className="empty__icon">📡</div>
              <div>No active agents</div>
              <div style={{ marginTop: 6, fontSize: 10, lineHeight: 1.7 }}>
                <code style={{ color: 'var(--color-thinking)' }}>ding claude "task"</code><br />
                <code style={{ color: 'var(--color-running)' }}>ding codex "fix bug"</code>
              </div>
            </div>
          ) : (
            sortedInstances.map((inst) => (
              <InstanceCard key={inst.id} instance={inst} />
            ))
          )}

          {/* Footer */}
          {sortedInstances.length > 0 && (
            <div className="panel__footer">
              <span>
                {sortedInstances.filter(i => i.status !== 'finished').length} active
                {sortedInstances.some(i => i.status === 'finished') &&
                  ` · ${sortedInstances.filter(i => i.status === 'finished').length} done`}
              </span>
              {totalCost > 0 && <span>Total ${totalCost.toFixed(3)}</span>}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default App;
