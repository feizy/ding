import { create } from 'zustand';
import type { Instance, DingStatus, PendingAction } from '../types/instance';

// Empty initial state for production
const INITIAL_INSTANCES: Instance[] = [];

interface InstanceStore {
  instances: Instance[];
  expanded: boolean;
  selectedInstanceId: string | null;
  primaryStatus: DingStatus;
  actionRequiredCount: number;

  setExpanded: (v: boolean) => void;
  toggleExpanded: () => void;
  selectInstance: (id: string | null) => void;
  setInstances: (instances: Instance[]) => void;
  upsertInstance: (instance: Instance) => void;
  updateInstanceStatus: (id: string, status: DingStatus) => void;
  updatePendingAction: (id: string, action: PendingAction) => void;
  appendLog: (id: string, log: any) => void;
  updateCost: (id: string, cost_usd: number) => void;
  clearPendingAction: (id: string) => void;
  removeInstance: (id: string) => void;
}

function computePrimaryStatus(instances: Instance[]): DingStatus {
  if (instances.length === 0) return 'idle';
  const priority: DingStatus[] = ['action_required', 'error', 'thinking', 'running', 'idle', 'finished'];
  for (const s of priority) {
    if (instances.some(i => i.status === s)) return s;
  }
  return 'idle';
}

export const useInstanceStore = create<InstanceStore>((set, get) => ({
  instances: INITIAL_INSTANCES,
  expanded: false,
  selectedInstanceId: null,
  primaryStatus: computePrimaryStatus(INITIAL_INSTANCES),
  actionRequiredCount: 0,

  setExpanded: (v) => set({ expanded: v }),
  toggleExpanded: () => set({ expanded: !get().expanded }),
  selectInstance: (id) => set({ selectedInstanceId: id }),

  setInstances: (instances) =>
    set({
      instances,
      primaryStatus: computePrimaryStatus(instances),
      actionRequiredCount: instances.filter(i => i.status === 'action_required').length,
    }),

  updateInstanceStatus: (id, status) =>
    set((state) => {
      const instances = state.instances.map(i =>
        i.id === id ? { ...i, status, last_event_at: new Date().toISOString() } : i
      );
      return {
        instances,
        primaryStatus: computePrimaryStatus(instances),
        actionRequiredCount: instances.filter(i => i.status === 'action_required').length,
      };
    }),

  updatePendingAction: (id, action) =>
    set((state) => {
      const instances = state.instances.map(i =>
        i.id === id ? { ...i, pending_action: action, status: 'action_required' as DingStatus, last_event_at: new Date().toISOString() } : i
      );
      return {
        instances,
        primaryStatus: computePrimaryStatus(instances),
        actionRequiredCount: instances.filter(i => i.status === 'action_required').length,
      };
    }),

  appendLog: (id, log) =>
    set((state) => {
      const instances = state.instances.map(i => {
        if (i.id !== id) return i;
        const recent_logs = [...i.recent_logs, log].slice(-100);
        return { ...i, recent_logs, last_event_at: new Date().toISOString() };
      });
      return { instances };
    }),

  updateCost: (id, cost_usd) =>
    set((state) => {
      const instances = state.instances.map(i =>
        i.id === id ? { ...i, cost_usd } : i
      );
      return { instances };
    }),

  upsertInstance: (instance) =>
    set((state) => {
      const exists = state.instances.some(i => i.id === instance.id);
      const instances = exists 
        ? state.instances.map(i => i.id === instance.id ? instance : i)
        : [...state.instances, instance];

      return {
        instances,
        primaryStatus: computePrimaryStatus(instances),
        actionRequiredCount: instances.filter(i => i.status === 'action_required').length,
      };
    }),

  clearPendingAction: (id) =>
    set((state) => {
      const instances = state.instances.map(i =>
        i.id === id
          ? { ...i, pending_action: null, status: 'running' as DingStatus }
          : i
      );
      return {
        instances,
        primaryStatus: computePrimaryStatus(instances),
        actionRequiredCount: instances.filter(i => i.status === 'action_required').length,
      };
    }),

  removeInstance: (id) =>
    set((state) => {
      const instances = state.instances.filter(i => i.id !== id);
      return {
        instances,
        selectedInstanceId: state.selectedInstanceId === id ? null : state.selectedInstanceId,
        primaryStatus: computePrimaryStatus(instances),
        actionRequiredCount: instances.filter(i => i.status === 'action_required').length,
      };
    }),
}));
