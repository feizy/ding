import { useCallback, useEffect, useLayoutEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { InstanceCard } from './components/InstanceCard';
import { StatusDot } from './components/StatusDot';
import { useInstanceStore } from './stores/instanceStore';
import type { Instance } from './types/instance';

const statusLabel: Record<string, string> = {
  idle: 'All idle',
  thinking: 'Thinking',
  tool_calling: 'Using tool',
  running: 'Running',
  action_required: 'Action needed',
  error: 'Error detected',
  finished: 'All done',
};

const priority: Record<string, number> = {
  action_required: 0,
  error: 1,
  thinking: 2,
  tool_calling: 3,
  running: 4,
  idle: 5,
  finished: 6,
};

const noDragSelector = [
  'button',
  'input',
  'textarea',
  'select',
  'a',
  '[role="button"]',
  '[data-no-drag="true"]',
].join(',');

function App() {
  const widgetRef = useRef<HTMLDivElement | null>(null);
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
      .catch(() => {});
  }, [setInstances]);

  const handleWindowDrag = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;

    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    if (target.closest(noDragSelector)) return;

    getCurrentWindow().startDragging().catch(() => {});
  }, []);

  useEffect(() => {
    refreshInstances();

    const pollId = setInterval(refreshInstances, 5000);

    const unlisten = listen('ding-event', (event: { payload: any }) => {
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
      unlisten.then((fn) => fn());
    };
  }, [
    appendLog,
    refreshInstances,
    removeInstance,
    setInstances,
    updateCost,
    updateInstanceStatus,
    updatePendingAction,
    upsertInstance,
  ]);

  useLayoutEffect(() => {
    const element = widgetRef.current;
    if (!element) return;

    const resizeToContent = () => {
      const rect = element.getBoundingClientRect();
      invoke('resize_widget', {
        width: Math.ceil(rect.width),
        height: Math.ceil(rect.height),
      }).catch(() => {});
    };

    resizeToContent();

    const observer = new ResizeObserver(() => resizeToContent());
    observer.observe(element);

    return () => observer.disconnect();
  }, [expanded, instances.length, actionRequiredCount]);

  const totalCost = instances.reduce((sum, instance) => sum + (instance.cost_usd ?? 0), 0);

  const sortedInstances = [...instances].sort((a, b) => {
    const aPriority = priority[a.status] ?? 99;
    const bPriority = priority[b.status] ?? 99;
    if (aPriority !== bPriority) return aPriority - bPriority;
    return new Date(b.last_event_at).getTime() - new Date(a.last_event_at).getTime();
  });

  const primaryInstance = sortedInstances[0] ?? null;
  const subtitle =
    actionRequiredCount > 0
      ? `${actionRequiredCount} action${actionRequiredCount > 1 ? 's' : ''} needed`
      : primaryInstance
      ? primaryStatus === 'tool_calling' && primaryInstance.current_tool_name
        ? `${primaryInstance.adapter_label} · ${primaryInstance.current_tool_name}`
        : `${primaryInstance.adapter_label} · ${statusLabel[primaryStatus] || ''}`
      : 'No active agents';

  const widgetClass = [
    'widget',
    primaryStatus === 'action_required' ? 'widget--action-required' : '',
    primaryStatus === 'error' ? 'widget--error' : '',
    expanded ? 'widget--expanded' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div
      ref={widgetRef}
      className={widgetClass}
      onPointerDown={handleWindowDrag}
    >
      <div className="capsule" onClick={toggleExpanded} data-tauri-drag-region>
        <StatusDot status={primaryStatus} />

        <div className="capsule__info">
          <div className="capsule__name">
            {primaryInstance ? primaryInstance.name : 'ding'}
            {totalCost > 0 && <span className="capsule__cost">${totalCost.toFixed(3)}</span>}
          </div>
          <div className="capsule__subtitle">{subtitle}</div>
        </div>

        <div className="capsule__right">
          {sortedInstances.length > 1 && (
            <div className="capsule__dots">
              {sortedInstances.slice(0, 5).map((instance) => (
                <StatusDot key={instance.id} status={instance.status} size="small" />
              ))}
              {sortedInstances.length > 5 && (
                <span className="capsule__more-count">+{sortedInstances.length - 5}</span>
              )}
            </div>
          )}
          <span
            className={`capsule__count ${
              actionRequiredCount > 0 ? 'capsule__count--urgent' : ''
            }`}
          >
            {actionRequiredCount > 0
              ? `${actionRequiredCount}`
              : sortedInstances.length > 0
              ? `${sortedInstances.length}`
              : '-'}
          </span>
          <span className={`capsule__chevron ${expanded ? 'capsule__chevron--up' : ''}`}>
            ▼
          </span>
        </div>
      </div>

      {expanded && (
        <div className="panel">
          {sortedInstances.length === 0 ? (
            <div className="empty">
              <div className="empty__icon">•</div>
              <div>No active agents</div>
            </div>
          ) : (
            sortedInstances.map((instance) => (
              <InstanceCard key={instance.id} instance={instance} />
            ))
          )}

          {sortedInstances.length > 0 && (
            <div className="panel__footer">
              <span>
                {
                  sortedInstances.filter((instance) => instance.status !== 'finished').length
                }{' '}
                active
                {sortedInstances.some((instance) => instance.status === 'finished') &&
                  ` · ${
                    sortedInstances.filter((instance) => instance.status === 'finished').length
                  } done`}
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
