use std::collections::HashMap;

use crate::instance::model::*;

/// Manages all active instances, provides sorted access by priority
pub struct InstanceManager {
    instances: HashMap<String, Instance>,
    pub decision_channels: HashMap<String, tokio::sync::mpsc::Sender<ActionDecision>>,
    counter: u32,
}

impl InstanceManager {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            decision_channels: HashMap::new(),
            counter: 0,
        }
    }

    /// Generate a short hex ID
    fn next_id(&mut self) -> String {
        self.counter += 1;
        format!("{:04x}", self.counter)
    }

    /// Create and register a new instance
    pub fn create_instance(&mut self, name: &str, adapter_type: AdapterType) -> String {
        let id = self.next_id();
        let instance = Instance::new(&id, name, adapter_type);
        self.instances.insert(id.clone(), instance);
        id
    }

    /// Get an instance by ID (mutable)
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Instance> {
        self.instances.get_mut(id)
    }

    /// Get an instance by ID
    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Option<&Instance> {
        self.instances.get(id)
    }

    /// Remove an instance
    pub fn remove(&mut self, id: &str) -> Option<Instance> {
        self.decision_channels.remove(id);
        self.instances.remove(id)
    }

    /// Return all instances sorted by priority (ActionRequired first, Finished last)
    pub fn sorted_instances(&self) -> Vec<&Instance> {
        let mut list: Vec<_> = self.instances.values().collect();
        list.sort_by(|a, b| {
            a.status
                .cmp(&b.status)
                .then(b.last_event_at.cmp(&a.last_event_at))
        });
        list
    }

    /// Convenience: the primary (highest priority) instance status
    pub fn primary_status(&self) -> DingStatus {
        self.sorted_instances()
            .first()
            .map(|i| i.status.clone())
            .unwrap_or(DingStatus::Idle)
    }

    pub fn count(&self) -> usize {
        self.instances.len()
    }

    pub fn action_required_count(&self) -> usize {
        self.instances
            .values()
            .filter(|i| i.status == DingStatus::ActionRequired)
            .count()
    }
}
