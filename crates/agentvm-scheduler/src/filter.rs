//! Filter plugins for scheduling

use crate::node::NodeInfo;
use crate::task::TaskSpec;

/// Result of applying a filter
#[derive(Debug, Clone)]
pub enum FilterResult {
    /// Node is acceptable
    Accept,
    /// Node is rejected with reason
    Reject(&'static str),
}

impl FilterResult {
    pub fn is_accept(&self) -> bool {
        matches!(self, Self::Accept)
    }

    pub fn is_reject(&self) -> bool {
        matches!(self, Self::Reject(_))
    }
}

/// Filter plugin trait
pub trait FilterPlugin: Send + Sync {
    /// Name of this filter
    fn name(&self) -> &str;

    /// Filter a node for a task
    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult;
}

/// Filter by tier affinity/anti-affinity
pub struct TierFilter;

impl FilterPlugin for TierFilter {
    fn name(&self) -> &str {
        "tier"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        // Check tier affinity
        if let Some(affinity) = &task.constraints.tier_affinity {
            if node.tier != *affinity {
                return FilterResult::Reject("tier affinity mismatch");
            }
        }

        // Check tier anti-affinity
        if task.constraints.tier_anti_affinity.contains(&node.tier) {
            return FilterResult::Reject("tier anti-affinity");
        }

        // Check if task class can run on this tier
        if !task.class.can_run_on(node.tier) {
            return FilterResult::Reject("task class incompatible with tier");
        }

        FilterResult::Accept
    }
}

/// Filter by resource availability
pub struct ResourceFilter;

impl FilterPlugin for ResourceFilter {
    fn name(&self) -> &str {
        "resource"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        // Check memory
        if task.resources.memory_bytes > node.available_memory {
            return FilterResult::Reject("insufficient memory");
        }

        // Check network
        if task.resources.network && !node.capabilities.network {
            return FilterResult::Reject("network not available");
        }

        // Check accelerator
        if let Some(accel) = &task.resources.accelerator {
            if !node.capabilities.accelerators.contains(accel) {
                return FilterResult::Reject("accelerator not available");
            }
        }

        FilterResult::Accept
    }
}

/// Filter by power budget
pub struct PowerFilter;

impl FilterPlugin for PowerFilter {
    fn name(&self) -> &str {
        "power"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        if let Some(budget) = task.constraints.power_budget_mw {
            if node.power_draw_mw > budget {
                return FilterResult::Reject("exceeds power budget");
            }
        }

        FilterResult::Accept
    }
}

/// Filter by capability requirements
pub struct CapabilityFilter;

impl FilterPlugin for CapabilityFilter {
    fn name(&self) -> &str {
        "capability"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        for cap in &task.capabilities {
            if !node.capabilities.capability_types.contains(cap) {
                return FilterResult::Reject("missing capability");
            }
        }

        FilterResult::Accept
    }
}

/// Filter by node affinity/anti-affinity
pub struct NodeAffinityFilter;

impl FilterPlugin for NodeAffinityFilter {
    fn name(&self) -> &str {
        "node_affinity"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        // Check node affinity (if specified, must be in list)
        if !task.constraints.node_affinity.is_empty() {
            if !task.constraints.node_affinity.contains(&node.id) {
                return FilterResult::Reject("node affinity mismatch");
            }
        }

        // Check node anti-affinity
        if task.constraints.node_anti_affinity.contains(&node.id) {
            return FilterResult::Reject("node anti-affinity");
        }

        FilterResult::Accept
    }
}

/// Filter by isolation level
pub struct IsolationFilter;

impl FilterPlugin for IsolationFilter {
    fn name(&self) -> &str {
        "isolation"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        // Node isolation level must be >= required level
        if node.capabilities.isolation_level < task.constraints.isolation {
            return FilterResult::Reject("insufficient isolation level");
        }

        FilterResult::Accept
    }
}

/// Filter by health status
pub struct HealthFilter;

impl FilterPlugin for HealthFilter {
    fn name(&self) -> &str {
        "health"
    }

    fn filter(&self, _task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        if !node.healthy {
            return FilterResult::Reject("node unhealthy");
        }

        FilterResult::Accept
    }
}

/// Composite filter combining multiple filters
pub struct CompositeFilter {
    filters: Vec<Box<dyn FilterPlugin>>,
}

impl CompositeFilter {
    pub fn new() -> Self {
        Self { filters: Vec::new() }
    }

    pub fn add(&mut self, filter: Box<dyn FilterPlugin>) {
        self.filters.push(filter);
    }

    /// Create with default filters
    pub fn with_defaults() -> Self {
        let mut composite = Self::new();
        composite.add(Box::new(HealthFilter));
        composite.add(Box::new(TierFilter));
        composite.add(Box::new(ResourceFilter));
        composite.add(Box::new(PowerFilter));
        composite.add(Box::new(CapabilityFilter));
        composite.add(Box::new(NodeAffinityFilter));
        composite.add(Box::new(IsolationFilter));
        composite
    }
}

impl Default for CompositeFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterPlugin for CompositeFilter {
    fn name(&self) -> &str {
        "composite"
    }

    fn filter(&self, task: &TaskSpec, node: &NodeInfo) -> FilterResult {
        for filter in &self.filters {
            match filter.filter(task, node) {
                FilterResult::Accept => continue,
                reject => return reject,
            }
        }
        FilterResult::Accept
    }
}
