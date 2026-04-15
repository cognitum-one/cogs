//! Filter plugins for the fabric scheduler.
//!
//! Filters eliminate infeasible nodes from consideration before scoring.
//! Each filter can accept or reject a node based on task requirements.

use crate::node::NodeInfo;
use crate::task::TaskSpec;

/// Result of a filter operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterResult {
    /// Node passes the filter.
    Accept,
    /// Node is rejected with a reason.
    Reject(String),
}

impl FilterResult {
    /// Check if the result is Accept.
    pub fn is_accept(&self) -> bool {
        matches!(self, FilterResult::Accept)
    }

    /// Check if the result is Reject.
    pub fn is_reject(&self) -> bool {
        matches!(self, FilterResult::Reject(_))
    }

    /// Get the rejection reason if rejected.
    pub fn rejection_reason(&self) -> Option<&str> {
        match self {
            FilterResult::Accept => None,
            FilterResult::Reject(reason) => Some(reason),
        }
    }
}

/// Trait for filter plugins.
///
/// Filter plugins eliminate infeasible nodes from consideration.
/// They should be fast and side-effect free.
pub trait FilterPlugin: Send + Sync {
    /// Get the name of this filter.
    fn name(&self) -> &str;

    /// Filter a node based on task requirements.
    ///
    /// Returns `FilterResult::Accept` if the node can potentially run the task,
    /// or `FilterResult::Reject(reason)` if it cannot.
    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult;
}

/// Filter nodes by tier compatibility.
///
/// Checks that the node's tier matches the task's requirements and constraints.
#[derive(Debug, Default)]
pub struct TierFilter;

impl FilterPlugin for TierFilter {
    fn name(&self) -> &str {
        "tier"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        // Check tier affinity constraint
        if let Some(affinity) = &task.constraints.tier_affinity {
            if node.tier != *affinity {
                return FilterResult::Reject(format!(
                    "tier affinity mismatch: required {:?}, got {:?}",
                    affinity, node.tier
                ));
            }
        }

        // Check tier anti-affinity constraint
        if task.constraints.tier_anti_affinity.contains(&node.tier) {
            return FilterResult::Reject(format!(
                "tier anti-affinity: {:?} is excluded",
                node.tier
            ));
        }

        // Check if task class can run on this tier
        if !task.class.can_run_on(node.tier) {
            return FilterResult::Reject(format!(
                "task class {:?} cannot run on tier {:?}",
                task.class, node.tier
            ));
        }

        FilterResult::Accept
    }
}

/// Filter nodes by resource availability.
///
/// Checks that the node has sufficient memory, network access, and accelerators.
#[derive(Debug, Default)]
pub struct ResourceFilter;

impl FilterPlugin for ResourceFilter {
    fn name(&self) -> &str {
        "resource"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        // Check memory
        if task.resources.memory_bytes > node.available_memory {
            return FilterResult::Reject(format!(
                "insufficient memory: required {} bytes, available {} bytes",
                task.resources.memory_bytes, node.available_memory
            ));
        }

        // Check network access
        if task.resources.network && !node.capabilities.network {
            return FilterResult::Reject("network access not available".to_string());
        }

        // Check accelerator
        if let Some(accel) = &task.resources.accelerator {
            if !node.capabilities.accelerators.contains(accel) {
                return FilterResult::Reject(format!(
                    "accelerator {:?} not available",
                    accel
                ));
            }
        }

        // Check task capacity
        if !node.can_accept_task() {
            return FilterResult::Reject("node at task capacity".to_string());
        }

        FilterResult::Accept
    }
}

/// Filter nodes by power budget.
///
/// Checks that the node's power consumption is within the task's power budget.
#[derive(Debug, Default)]
pub struct PowerFilter;

impl FilterPlugin for PowerFilter {
    fn name(&self) -> &str {
        "power"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        if let Some(budget) = task.constraints.power_budget_mw {
            if node.power_draw_mw > budget {
                return FilterResult::Reject(format!(
                    "exceeds power budget: {} mW > {} mW",
                    node.power_draw_mw, budget
                ));
            }
        }

        FilterResult::Accept
    }
}

/// Filter nodes by required capabilities.
///
/// Checks that the node provides all capabilities required by the task.
#[derive(Debug, Default)]
pub struct CapabilityFilter;

impl FilterPlugin for CapabilityFilter {
    fn name(&self) -> &str {
        "capability"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        for cap in &task.capabilities {
            if !node.capabilities.available_capabilities.contains(cap) {
                return FilterResult::Reject(format!(
                    "missing capability: {:?}",
                    cap
                ));
            }
        }

        FilterResult::Accept
    }
}

/// Filter nodes by isolation level.
///
/// Checks that the node provides the required security isolation level.
#[derive(Debug, Default)]
pub struct IsolationFilter;

impl FilterPlugin for IsolationFilter {
    fn name(&self) -> &str {
        "isolation"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        if !node.isolation_level.satisfies(task.constraints.isolation) {
            return FilterResult::Reject(format!(
                "insufficient isolation: required {:?}, got {:?}",
                task.constraints.isolation, node.isolation_level
            ));
        }

        FilterResult::Accept
    }
}

/// Filter nodes by node affinity/anti-affinity.
///
/// Checks that the node matches affinity constraints.
#[derive(Debug, Default)]
pub struct NodeAffinityFilter;

impl FilterPlugin for NodeAffinityFilter {
    fn name(&self) -> &str {
        "node-affinity"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        // Check node anti-affinity
        if task.constraints.node_anti_affinity.contains(&node.id) {
            return FilterResult::Reject("node is in anti-affinity list".to_string());
        }

        // Check node affinity (if specified, node must be in list)
        if !task.constraints.node_affinity.is_empty()
            && !task.constraints.node_affinity.contains(&node.id)
        {
            return FilterResult::Reject("node not in affinity list".to_string());
        }

        FilterResult::Accept
    }
}

/// Filter nodes by health status.
///
/// Checks that the node is healthy and can accept tasks.
#[derive(Debug, Default)]
pub struct HealthFilter;

impl FilterPlugin for HealthFilter {
    fn name(&self) -> &str {
        "health"
    }

    fn filter(&self, _task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        use crate::node::NodeHealth;

        match node.health {
            NodeHealth::Healthy => FilterResult::Accept,
            NodeHealth::Degraded => FilterResult::Accept, // Accept but may score lower
            NodeHealth::Unhealthy => {
                FilterResult::Reject("node is unhealthy".to_string())
            }
            NodeHealth::Offline => FilterResult::Reject("node is offline".to_string()),
        }
    }
}

/// Collection of all built-in filters.
pub struct FilterChain {
    filters: Vec<Box<dyn FilterPlugin>>,
}

impl FilterChain {
    /// Create a new filter chain with all default filters.
    pub fn default_chain() -> Self {
        Self {
            filters: vec![
                Box::new(HealthFilter),
                Box::new(TierFilter),
                Box::new(ResourceFilter),
                Box::new(PowerFilter),
                Box::new(CapabilityFilter),
                Box::new(IsolationFilter),
                Box::new(NodeAffinityFilter),
            ],
        }
    }

    /// Create an empty filter chain.
    pub fn empty() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Add a filter to the chain.
    pub fn add_filter(mut self, filter: Box<dyn FilterPlugin>) -> Self {
        self.filters.push(filter);
        self
    }

    /// Run all filters on a node.
    ///
    /// Returns the first rejection reason, or Accept if all filters pass.
    pub fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        for filter in &self.filters {
            let result = filter.filter(task, node);
            if let FilterResult::Reject(reason) = result {
                tracing::debug!(
                    filter = filter.name(),
                    node = %node.id,
                    reason = %reason,
                    "Node rejected by filter"
                );
                return FilterResult::Reject(format!("{}: {}", filter.name(), reason));
            }
        }
        FilterResult::Accept
    }

    /// Filter a list of nodes, returning only those that pass all filters.
    pub fn filter_nodes(&self, task: &TaskSpec, nodes: &[NodeInfo]) -> Vec<NodeInfo> {
        nodes
            .iter()
            .filter(|node| self.filter(task, node).is_accept())
            .cloned()
            .collect()
    }
}

impl Default for FilterChain {
    fn default() -> Self {
        Self::default_chain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{IsolationLevel, NodeHealth, Tier};
    use crate::task::{CapabilityType, CapsuleId, TaskClass};

    fn create_test_node(tier: Tier) -> NodeInfo {
        NodeInfo::new("test-node", tier)
    }

    fn create_test_task(class: TaskClass) -> TaskSpec {
        TaskSpec::new(CapsuleId::new(), class)
    }

    #[test]
    fn test_tier_filter_affinity() {
        let filter = TierFilter;
        let node = create_test_node(Tier::Host);

        // Task with matching tier affinity
        let mut task = create_test_task(TaskClass::Network);
        task.constraints.tier_affinity = Some(Tier::Host);
        assert!(filter.filter(&task, &node).is_accept());

        // Task with non-matching tier affinity
        task.constraints.tier_affinity = Some(Tier::Edge);
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_tier_filter_anti_affinity() {
        let filter = TierFilter;
        let node = create_test_node(Tier::Host);

        let mut task = create_test_task(TaskClass::Network);
        task.constraints.tier_anti_affinity = vec![Tier::Host];
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_tier_filter_task_class() {
        let filter = TierFilter;

        // Reflex task cannot run on host
        let host_node = create_test_node(Tier::Host);
        let reflex_task = create_test_task(TaskClass::Reflex);
        assert!(filter.filter(&reflex_task, &host_node).is_reject());

        // Network task can run on host
        let network_task = create_test_task(TaskClass::Network);
        assert!(filter.filter(&network_task, &host_node).is_accept());

        // Heavy compute cannot run on edge
        let edge_node = create_test_node(Tier::Edge);
        let heavy_task = create_test_task(TaskClass::HeavyCompute);
        assert!(filter.filter(&heavy_task, &edge_node).is_reject());
    }

    #[test]
    fn test_resource_filter_memory() {
        let filter = ResourceFilter;
        let mut node = create_test_node(Tier::Host);
        node.available_memory = 1024 * 1024; // 1 MB

        let mut task = create_test_task(TaskClass::Network);
        task.resources.memory_bytes = 512 * 1024; // 512 KB
        assert!(filter.filter(&task, &node).is_accept());

        task.resources.memory_bytes = 2 * 1024 * 1024; // 2 MB
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_resource_filter_network() {
        let filter = ResourceFilter;
        let mut node = create_test_node(Tier::Edge);
        node.capabilities.network = false;

        let mut task = create_test_task(TaskClass::Anomaly);
        task.resources.network = true;
        assert!(filter.filter(&task, &node).is_reject());

        task.resources.network = false;
        assert!(filter.filter(&task, &node).is_accept());
    }

    #[test]
    fn test_power_filter() {
        let filter = PowerFilter;
        let mut node = create_test_node(Tier::Host);
        node.power_draw_mw = 10000; // 10W

        let mut task = create_test_task(TaskClass::Network);
        task.constraints.power_budget_mw = Some(15000);
        assert!(filter.filter(&task, &node).is_accept());

        task.constraints.power_budget_mw = Some(5000);
        assert!(filter.filter(&task, &node).is_reject());

        // No power budget constraint
        task.constraints.power_budget_mw = None;
        assert!(filter.filter(&task, &node).is_accept());
    }

    #[test]
    fn test_capability_filter() {
        let filter = CapabilityFilter;
        let node = create_test_node(Tier::Host);

        let mut task = create_test_task(TaskClass::Network);
        task.capabilities = vec![CapabilityType::NetworkHttp];
        assert!(filter.filter(&task, &node).is_accept());

        task.capabilities = vec![CapabilityType::GpuCompute];
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_isolation_filter() {
        let filter = IsolationFilter;
        let mut node = create_test_node(Tier::Host);
        node.isolation_level = IsolationLevel::Vm;

        let mut task = create_test_task(TaskClass::Network);
        task.constraints.isolation = IsolationLevel::Process;
        assert!(filter.filter(&task, &node).is_accept());

        task.constraints.isolation = IsolationLevel::Hardware;
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_health_filter() {
        let filter = HealthFilter;
        let mut node = create_test_node(Tier::Host);
        let task = create_test_task(TaskClass::Network);

        node.health = NodeHealth::Healthy;
        assert!(filter.filter(&task, &node).is_accept());

        node.health = NodeHealth::Degraded;
        assert!(filter.filter(&task, &node).is_accept());

        node.health = NodeHealth::Unhealthy;
        assert!(filter.filter(&task, &node).is_reject());

        node.health = NodeHealth::Offline;
        assert!(filter.filter(&task, &node).is_reject());
    }

    #[test]
    fn test_filter_chain() {
        let chain = FilterChain::default_chain();

        let host_node = create_test_node(Tier::Host);
        let network_task = create_test_task(TaskClass::Network);
        assert!(chain.filter(&network_task, &host_node).is_accept());

        // Reflex task should be rejected on host
        let reflex_task = create_test_task(TaskClass::Reflex);
        assert!(chain.filter(&reflex_task, &host_node).is_reject());
    }

    #[test]
    fn test_filter_chain_filter_nodes() {
        let chain = FilterChain::default_chain();

        let edge_node = create_test_node(Tier::Edge);
        let host_node = create_test_node(Tier::Host);
        let nodes = vec![edge_node, host_node];

        // Network task should only pass on host
        let network_task = create_test_task(TaskClass::Network);
        let filtered = chain.filter_nodes(&network_task, &nodes);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].tier, Tier::Host);
    }
}
