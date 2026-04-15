//! Thermal-Graph Dynamic Overclocking with MinCut
//!
//! Models the SoC thermal system as a weighted graph where the source-sink
//! minimum cut equals the thermal bottleneck capacity between heat generators
//! and ambient air.
//!
//! ## Optimizations over naive approach
//!
//! - **Source-sink max-flow/min-cut** via push-relabel (not global Stoer-Wagner)
//!   to find the actual bottleneck between cores and ambient
//! - **Temperature-dependent conductance** scaling (silicon conductivity
//!   drops ~30% from 25C to 80C)
//! - **Incremental recomputation** skipped when max delta-T < threshold
//! - **Transient thermal simulation** via forward-Euler for unmeasured nodes
//! - **Online conductance calibration** adjusting edge weights from observed
//!   vs predicted temperature deltas
//!
//! Reference: ADR-032, Goldberg-Tarjan push-relabel (1988)

/// Maximum thermal nodes in the graph (13 for Pi Zero 2W full model)
pub const MAX_THERMAL_NODES: usize = 16;

/// Maximum edges in the thermal graph (bidirectional, so 2x physical paths)
pub const MAX_THERMAL_EDGES: usize = 64;

/// Maximum frequency actions for the governor
pub const MAX_FREQ_ACTIONS: usize = 8;

/// Type of thermal node
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThermalNodeType {
    /// Heat-generating component (CPU core, GPU)
    HeatSource,
    /// Thermal conductor (die, PCB, DRAM package)
    Conductor,
    /// Heat sink to environment (ambient air)
    HeatSink,
}

/// A node in the thermal conductance graph
#[derive(Clone, Copy, Debug)]
pub struct ThermalNode {
    /// Node identifier (0-based index)
    pub id: u8,
    /// Current temperature in Celsius
    pub temp_c: f32,
    /// Previous temperature for delta tracking
    prev_temp_c: f32,
    /// Thermal mass in J/K (how quickly it heats/cools)
    pub thermal_mass: f32,
    /// Power generation in watts (0 for non-source nodes)
    pub power_w: f32,
    /// Node type classification
    pub node_type: ThermalNodeType,
    /// Whether this node has a direct sensor
    pub has_sensor: bool,
    /// Whether this node is active
    pub active: bool,
}

impl Default for ThermalNode {
    fn default() -> Self {
        Self {
            id: 0,
            temp_c: 25.0,
            prev_temp_c: 25.0,
            thermal_mass: 1.0,
            power_w: 0.0,
            node_type: ThermalNodeType::Conductor,
            has_sensor: false,
            active: false,
        }
    }
}

/// Edge in the thermal conductance graph
#[derive(Clone, Copy, Debug, Default)]
pub struct ThermalEdge {
    /// Source node ID
    pub from: u8,
    /// Destination node ID
    pub to: u8,
    /// Base thermal conductance in W/K (at 25C reference)
    pub base_conductance: f32,
    /// Current effective conductance (temperature-adjusted)
    pub conductance: f32,
}

/// Temperature-dependent conductance model
///
/// Silicon thermal conductivity: k(T) = k_ref * (T_ref / T)^alpha
/// where alpha ~ 1.3 for silicon, T in Kelvin.
/// For simplified embedded use: G(T) = G_base * (1 - beta * (T - 25))
/// where beta ~ 0.004 /C captures the ~30% drop from 25C to 80C.
const CONDUCTANCE_TEMP_COEFF: f32 = 0.004;

/// Minimum delta-T (max across all sensed nodes) to trigger recomputation
const RECOMPUTE_THRESHOLD_C: f32 = 0.5;

/// Thermal conductance graph with source-sink min-cut analysis
pub struct ThermalGraph {
    /// Graph nodes (thermal zones)
    nodes: [ThermalNode; MAX_THERMAL_NODES],
    /// Graph edges (thermal conductance paths)
    edges: [ThermalEdge; MAX_THERMAL_EDGES],
    /// Number of active nodes
    num_nodes: usize,
    /// Number of active edges
    num_edges: usize,
    /// Adjacency matrix (effective conductance), rebuilt from edges
    adj: [[f32; MAX_THERMAL_NODES]; MAX_THERMAL_NODES],
    /// Cached min-cut value (W/K)
    cached_mincut: f32,
    /// Whether cache is valid
    cache_valid: bool,
    /// Super-source node ID (virtual, connects all HeatSource nodes)
    super_source: u8,
    /// Super-sink node ID (virtual, connects all HeatSink nodes)
    super_sink: u8,
}

impl ThermalGraph {
    /// Create an empty thermal graph
    pub fn new() -> Self {
        Self {
            nodes: [ThermalNode::default(); MAX_THERMAL_NODES],
            edges: [ThermalEdge::default(); MAX_THERMAL_EDGES],
            num_nodes: 0,
            num_edges: 0,
            adj: [[0.0; MAX_THERMAL_NODES]; MAX_THERMAL_NODES],
            cached_mincut: 0.0,
            cache_valid: false,
            super_source: 14,
            super_sink: 15,
        }
    }

    /// Create the BCM2710A1 (Pi Zero 2W) thermal graph
    ///
    /// Nodes:
    ///   0-3: CPU cores (Cortex-A53) - sensored via /sys/class/thermal
    ///   4: L2 cache (interpolated)
    ///   5: VideoCore IV GPU (sensored)
    ///   6: Die substrate (interpolated)
    ///   7: DRAM PoP (interpolated)
    ///   8: PCB top copper (interpolated)
    ///   9: PCB bottom copper (interpolated)
    ///   10: WiFi/BT IC CYW43438 (interpolated)
    ///   11: Air top
    ///   12: Air bottom
    ///   14: Super-source (virtual)
    ///   15: Super-sink (virtual)
    pub fn bcm2710a1() -> Self {
        let mut g = Self::new();

        // Physical nodes
        g.add_node_full(0, 25.0, 0.012, 0.0, ThermalNodeType::HeatSource, true);
        g.add_node_full(1, 25.0, 0.012, 0.0, ThermalNodeType::HeatSource, true);
        g.add_node_full(2, 25.0, 0.012, 0.0, ThermalNodeType::HeatSource, true);
        g.add_node_full(3, 25.0, 0.012, 0.0, ThermalNodeType::HeatSource, true);
        g.add_node_full(4, 25.0, 0.020, 0.0, ThermalNodeType::Conductor, false);
        g.add_node_full(5, 25.0, 0.015, 0.0, ThermalNodeType::HeatSource, true);
        g.add_node_full(6, 25.0, 0.050, 0.0, ThermalNodeType::Conductor, false);
        g.add_node_full(7, 25.0, 0.030, 0.0, ThermalNodeType::Conductor, false);
        g.add_node_full(8, 25.0, 0.200, 0.0, ThermalNodeType::Conductor, false);
        g.add_node_full(9, 25.0, 0.200, 0.0, ThermalNodeType::Conductor, false);
        g.add_node_full(10, 25.0, 0.010, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(11, 25.0, f32::MAX, 0.0, ThermalNodeType::HeatSink, true);
        g.add_node_full(12, 25.0, f32::MAX, 0.0, ThermalNodeType::HeatSink, true);

        // Virtual super-source/sink
        g.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);

        // Physical thermal paths
        g.add_edge(0, 4, 1.5);  // Core0 -> L2
        g.add_edge(1, 4, 1.5);  // Core1 -> L2
        g.add_edge(2, 4, 1.5);  // Core2 -> L2
        g.add_edge(3, 4, 1.5);  // Core3 -> L2
        g.add_edge(0, 1, 0.6);  // Core0 <-> Core1 lateral
        g.add_edge(2, 3, 0.6);  // Core2 <-> Core3 lateral
        g.add_edge(4, 6, 2.0);  // L2 -> Die
        g.add_edge(5, 6, 1.8);  // GPU -> Die
        g.add_edge(6, 7, 0.4);  // Die -> DRAM (PoP solder, likely bottleneck)
        g.add_edge(6, 8, 1.0);  // Die -> PCB top (thermal vias)
        g.add_edge(8, 9, 0.8);  // PCB top -> bottom (through-board vias)
        g.add_edge(7, 11, 0.25); // DRAM -> Air top
        g.add_edge(8, 11, 0.15); // PCB top -> Air top
        g.add_edge(9, 12, 0.20); // PCB bottom -> Air bottom
        g.add_edge(10, 8, 0.5); // WiFi IC -> PCB top

        // Super-source edges (infinite capacity to all heat sources)
        g.add_edge(14, 0, 100.0);
        g.add_edge(14, 1, 100.0);
        g.add_edge(14, 2, 100.0);
        g.add_edge(14, 3, 100.0);
        g.add_edge(14, 5, 100.0);
        g.add_edge(14, 10, 100.0);

        // Super-sink edges (infinite capacity from all heat sinks)
        g.add_edge(11, 15, 100.0);
        g.add_edge(12, 15, 100.0);

        g.rebuild_adjacency();
        g
    }

    /// Add a node with full parameters
    fn add_node_full(
        &mut self,
        id: u8,
        temp_c: f32,
        thermal_mass: f32,
        power_w: f32,
        node_type: ThermalNodeType,
        has_sensor: bool,
    ) {
        let idx = id as usize;
        if idx < MAX_THERMAL_NODES {
            self.nodes[idx] = ThermalNode {
                id,
                temp_c,
                prev_temp_c: temp_c,
                thermal_mass,
                power_w,
                node_type,
                has_sensor,
                active: true,
            };
            if idx >= self.num_nodes {
                self.num_nodes = idx + 1;
            }
        }
    }

    /// Add a node (simplified API, backward compatible)
    pub fn add_node(
        &mut self,
        id: u8,
        temp_c: f32,
        thermal_mass: f32,
        node_type: ThermalNodeType,
    ) {
        self.add_node_full(id, temp_c, thermal_mass, 0.0, node_type, false);
        self.cache_valid = false;
    }

    /// Add an edge (thermal conductance path)
    pub fn add_edge(&mut self, from: u8, to: u8, conductance: f32) {
        if self.num_edges < MAX_THERMAL_EDGES {
            self.edges[self.num_edges] = ThermalEdge {
                from,
                to,
                base_conductance: conductance,
                conductance,
            };
            self.num_edges += 1;
            self.cache_valid = false;
        }
    }

    /// Update a node's temperature from sensor reading
    pub fn update_temp(&mut self, node_id: u8, temp_c: f32) {
        let idx = node_id as usize;
        if idx < self.num_nodes && self.nodes[idx].active {
            self.nodes[idx].prev_temp_c = self.nodes[idx].temp_c;
            self.nodes[idx].temp_c = temp_c;
        }
    }

    /// Update an edge's base conductance (e.g., heatsink added/removed)
    pub fn update_conductance(&mut self, from: u8, to: u8, conductance: f32) {
        for i in 0..self.num_edges {
            let e = &self.edges[i];
            if (e.from == from && e.to == to) || (e.from == to && e.to == from) {
                self.edges[i].base_conductance = conductance;
                self.edges[i].conductance = conductance;
                self.cache_valid = false;
                return;
            }
        }
    }

    /// Apply temperature-dependent conductance scaling to all edges.
    ///
    /// Silicon thermal conductivity drops with temperature:
    /// G_eff = G_base * (1 - beta * (T_avg_endpoints - 25))
    fn scale_conductances(&mut self) {
        for i in 0..self.num_edges {
            let e = &self.edges[i];
            let u = e.from as usize;
            let v = e.to as usize;
            if u < self.num_nodes && v < self.num_nodes {
                let t_avg = (self.nodes[u].temp_c + self.nodes[v].temp_c) * 0.5;
                let scale = (1.0 - CONDUCTANCE_TEMP_COEFF * (t_avg - 25.0)).clamp(0.3, 1.2);
                self.edges[i].conductance = self.edges[i].base_conductance * scale;
            }
        }
    }

    /// Rebuild adjacency matrix from edge list
    fn rebuild_adjacency(&mut self) {
        self.adj = [[0.0; MAX_THERMAL_NODES]; MAX_THERMAL_NODES];
        for i in 0..self.num_edges {
            let e = &self.edges[i];
            let u = e.from as usize;
            let v = e.to as usize;
            if u < MAX_THERMAL_NODES && v < MAX_THERMAL_NODES {
                self.adj[u][v] += e.conductance;
                self.adj[v][u] += e.conductance;
            }
        }
    }

    /// Propagate temperatures for unmeasured nodes using forward-Euler.
    ///
    /// For each node without a sensor:
    ///   dT/dt = (1/C) * [sum_neighbors(G_ij * (T_j - T_i)) + P_gen]
    ///
    /// Uses dt_s as the simulation timestep.
    pub fn simulate_step(&mut self, dt_s: f32) {
        // Collect temperature deltas first (avoid borrow issues)
        let mut dt_arr = [0.0f32; MAX_THERMAL_NODES];

        for i in 0..self.num_nodes {
            let node = &self.nodes[i];
            if !node.active || node.has_sensor || node.thermal_mass <= 0.0
                || node.thermal_mass == f32::MAX
            {
                continue;
            }

            let mut heat_flow = node.power_w;
            for j in 0..self.num_nodes {
                if i == j || !self.nodes[j].active {
                    continue;
                }
                let g = self.adj[i][j];
                if g > 0.0 {
                    heat_flow += g * (self.nodes[j].temp_c - self.nodes[i].temp_c);
                }
            }
            dt_arr[i] = heat_flow / node.thermal_mass * dt_s;
        }

        // Apply deltas
        for i in 0..self.num_nodes {
            if dt_arr[i] != 0.0 {
                self.nodes[i].prev_temp_c = self.nodes[i].temp_c;
                self.nodes[i].temp_c += dt_arr[i];
            }
        }
    }

    /// Check if any sensed node changed enough to warrant recomputation
    fn needs_recompute(&self) -> bool {
        for i in 0..self.num_nodes {
            let n = &self.nodes[i];
            if n.active && n.has_sensor {
                let delta = (n.temp_c - n.prev_temp_c).abs();
                if delta >= RECOMPUTE_THRESHOLD_C {
                    return true;
                }
            }
        }
        false
    }

    /// Compute source-sink min-cut using push-relabel max-flow.
    ///
    /// The max-flow from super-source (all cores) to super-sink (all ambient)
    /// equals the min-cut by the max-flow min-cut theorem.
    ///
    /// Returns the min-cut value in W/K.
    pub fn compute_mincut(&mut self) -> f32 {
        if self.cache_valid && !self.needs_recompute() {
            return self.cached_mincut;
        }

        self.scale_conductances();
        self.rebuild_adjacency();

        let n = self.num_nodes.max(self.super_sink as usize + 1);
        let s = self.super_source as usize;
        let t = self.super_sink as usize;

        if s >= n || t >= n || s == t {
            self.cached_mincut = 0.0;
            self.cache_valid = true;
            return 0.0;
        }

        // Push-relabel (FIFO variant) for max-flow
        // Residual capacity
        let mut cap = [[0.0f32; MAX_THERMAL_NODES]; MAX_THERMAL_NODES];
        for i in 0..n {
            for j in 0..n {
                cap[i][j] = self.adj[i][j];
            }
        }

        let mut excess = [0.0f32; MAX_THERMAL_NODES];
        let mut height = [0u32; MAX_THERMAL_NODES];
        height[s] = n as u32;

        // Initial preflow: saturate all edges from source
        for v in 0..n {
            if cap[s][v] > 0.0 {
                let flow = cap[s][v];
                cap[s][v] -= flow;
                cap[v][s] += flow;
                excess[v] += flow;
                excess[s] -= flow;
            }
        }

        // FIFO queue (circular buffer using fixed array)
        let mut queue = [0u8; MAX_THERMAL_NODES];
        let mut q_head = 0usize;
        let mut q_tail = 0usize;
        let mut in_queue = [false; MAX_THERMAL_NODES];

        for v in 0..n {
            if v != s && v != t && excess[v] > 1e-9 {
                queue[q_tail] = v as u8;
                q_tail = (q_tail + 1) % MAX_THERMAL_NODES;
                in_queue[v] = true;
            }
        }

        // Max iterations: O(V^2 * E) for push-relabel; generous for V=16
        let max_iters = n * n * 8;
        let mut iters = 0;

        while q_head != q_tail && iters < max_iters {
            let u = queue[q_head] as usize;
            q_head = (q_head + 1) % MAX_THERMAL_NODES;
            in_queue[u] = false;

            // Discharge u: push along admissible arcs, relabel when stuck
            while excess[u] > 1e-9 && iters < max_iters {
                iters += 1;

                // Push to all admissible neighbors
                for v in 0..n {
                    if cap[u][v] > 1e-9 && height[u] == height[v] + 1 {
                        let d = excess[u].min(cap[u][v]);
                        cap[u][v] -= d;
                        cap[v][u] += d;
                        excess[u] -= d;
                        excess[v] += d;

                        if v != s && v != t && !in_queue[v] && excess[v] > 1e-9 {
                            queue[q_tail] = v as u8;
                            q_tail = (q_tail + 1) % MAX_THERMAL_NODES;
                            in_queue[v] = true;
                        }
                        if excess[u] <= 1e-9 {
                            break;
                        }
                    }
                }

                if excess[u] <= 1e-9 {
                    break;
                }

                // Relabel: raise height to min(neighbor heights with residual) + 1
                let mut min_h = u32::MAX;
                for v in 0..n {
                    if cap[u][v] > 1e-9 && height[v] < min_h {
                        min_h = height[v];
                    }
                }
                if min_h < u32::MAX {
                    height[u] = min_h + 1;
                } else {
                    break; // No residual path to sink
                }
            }
        }

        // Max-flow = total excess at sink = min-cut
        let max_flow = excess[t];
        self.cached_mincut = if max_flow > 0.0 { max_flow } else { 0.0 };
        self.cache_valid = true;
        self.cached_mincut
    }

    /// Get the cached min-cut value without recomputing
    pub fn mincut(&self) -> f32 {
        self.cached_mincut
    }

    /// Compute maximum sustainable heat dissipation in watts
    ///
    /// Q_max = mincut_conductance * (T_junction_max - T_ambient)
    pub fn max_dissipation(&mut self, t_junction_max_c: f32, t_ambient_c: f32) -> f32 {
        let mincut = self.compute_mincut();
        let delta_t = t_junction_max_c - t_ambient_c;
        if delta_t > 0.0 { mincut * delta_t } else { 0.0 }
    }

    /// Online conductance calibration.
    ///
    /// Compares predicted vs observed temperature change for sensed nodes
    /// and adjusts base conductance of adjacent edges. Call after
    /// `simulate_step` + `update_temp` with fresh sensor data.
    pub fn calibrate(&mut self, learning_rate: f32) {
        for i in 0..self.num_nodes {
            let node = &self.nodes[i];
            if !node.active || !node.has_sensor {
                continue;
            }
            // prediction error = actual_delta - simulated_delta would require
            // storing the simulated value. Simplified: if node is hotter than
            // neighbors predict, conductance is lower than modeled.
            let mut neighbor_avg = 0.0f32;
            let mut neighbor_g = 0.0f32;
            for j in 0..self.num_nodes {
                let g = self.adj[i][j];
                if g > 0.0 && self.nodes[j].active {
                    neighbor_avg += self.nodes[j].temp_c * g;
                    neighbor_g += g;
                }
            }
            if neighbor_g <= 0.0 {
                continue;
            }
            neighbor_avg /= neighbor_g;

            let error = node.temp_c - neighbor_avg;
            // If node is hotter than weighted-average neighbor, conductance
            // from node to cooler side is lower than modeled -> reduce.
            // Clamp adjustment to prevent runaway.
            let adj_factor = 1.0 - (error * learning_rate).clamp(-0.1, 0.1);

            for e_idx in 0..self.num_edges {
                let e = &self.edges[e_idx];
                if e.from == node.id || e.to == node.id {
                    self.edges[e_idx].base_conductance *= adj_factor;
                    // Floor at 10% of original to prevent collapse
                    if self.edges[e_idx].base_conductance < 0.01 {
                        self.edges[e_idx].base_conductance = 0.01;
                    }
                }
            }
        }
        self.cache_valid = false;
    }

    /// Get number of active nodes
    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    /// Get number of edges
    pub fn num_edges(&self) -> usize {
        self.num_edges
    }

    /// Get a node's current temperature
    pub fn node_temp(&self, id: u8) -> f32 {
        let idx = id as usize;
        if idx < self.num_nodes { self.nodes[idx].temp_c } else { 0.0 }
    }

    /// Get average temperature of heat source nodes (excluding virtual)
    pub fn avg_source_temp(&self) -> f32 {
        let mut sum = 0.0f32;
        let mut count = 0u32;
        for i in 0..self.num_nodes {
            let n = &self.nodes[i];
            if n.active
                && n.node_type == ThermalNodeType::HeatSource
                && n.id != self.super_source
            {
                sum += n.temp_c;
                count += 1;
            }
        }
        if count > 0 { sum / count as f32 } else { 25.0 }
    }

    /// Invalidate cache (force recomputation on next compute_mincut)
    pub fn invalidate(&mut self) {
        self.cache_valid = false;
    }
}

impl Default for ThermalGraph {
    fn default() -> Self {
        Self::new()
    }
}

// --- MinCut Governor ---

/// Discrete CPU frequency target in MHz
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreqAction {
    /// Frequency in MHz
    pub freq_mhz: u32,
    /// Estimated power draw in watts (at full load)
    pub power_w: f32,
}

/// MinCut-driven frequency governor configuration
#[derive(Clone, Copy, Debug)]
pub struct MinCutGovernorConfig {
    /// Min-cut threshold below which to throttle (W/K)
    pub throttle_threshold: f32,
    /// Min-cut threshold for baseline operation (W/K)
    pub baseline_threshold: f32,
    /// Min-cut threshold above which burst is allowed (W/K)
    pub burst_threshold: f32,
    /// Maximum burst duration in milliseconds
    pub max_burst_ms: u32,
    /// Cooldown period after burst in milliseconds
    pub cooldown_ms: u32,
    /// Ambient temperature in Celsius
    pub ambient_c: f32,
    /// Maximum junction temperature in Celsius
    pub t_junction_max_c: f32,
    /// EMA alpha for min-cut smoothing (higher = more responsive)
    pub ema_alpha: f32,
    /// Conductance calibration learning rate (0 = disabled)
    pub calibration_rate: f32,
}

impl Default for MinCutGovernorConfig {
    fn default() -> Self {
        Self {
            throttle_threshold: 0.25,
            baseline_threshold: 0.35,
            burst_threshold: 0.50,
            max_burst_ms: 200,
            cooldown_ms: 800,
            ambient_c: 25.0,
            t_junction_max_c: 80.0,
            ema_alpha: 0.3,
            calibration_rate: 0.01,
        }
    }
}

/// Governor state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinCutState {
    /// Throttled to minimum frequency
    Throttle,
    /// Running at baseline frequency
    Baseline,
    /// Burst overclocking active
    Burst,
    /// Cooling down after burst
    Cooldown,
}

/// MinCut-driven dynamic frequency governor
///
/// Uses thermal graph source-sink min-cut to determine safe burst headroom
/// and drives CPU frequency selection accordingly. Integrates transient
/// thermal simulation for unmeasured nodes and online conductance calibration.
pub struct MinCutGovernor {
    config: MinCutGovernorConfig,
    graph: ThermalGraph,
    state: MinCutState,
    /// Time in current state (ms)
    state_time_ms: u32,
    /// Available frequency actions (sorted ascending by freq)
    freq_actions: [FreqAction; MAX_FREQ_ACTIONS],
    num_actions: usize,
    /// Currently selected action index
    current_action: usize,
    /// Min-cut value EMA
    mincut_ema: f32,
    /// Burst count (statistics)
    burst_count: u32,
    /// Total burst time ms (statistics)
    total_burst_ms: u64,
    /// Whether EMA has been seeded
    ema_seeded: bool,
}

impl MinCutGovernor {
    /// Create a new MinCut governor with given config and thermal graph
    pub fn new(config: MinCutGovernorConfig, graph: ThermalGraph) -> Self {
        let freq_actions = [
            FreqAction { freq_mhz: 600, power_w: 1.5 },
            FreqAction { freq_mhz: 800, power_w: 2.2 },
            FreqAction { freq_mhz: 1000, power_w: 3.0 },
            FreqAction { freq_mhz: 1200, power_w: 3.8 },
            FreqAction { freq_mhz: 1300, power_w: 4.2 },
            FreqAction { freq_mhz: 1500, power_w: 5.5 },
            FreqAction { freq_mhz: 1600, power_w: 6.2 },
            FreqAction { freq_mhz: 1700, power_w: 7.0 },
        ];

        Self {
            config,
            graph,
            state: MinCutState::Baseline,
            state_time_ms: 0,
            freq_actions,
            num_actions: 8,
            current_action: 2,
            mincut_ema: 0.0,
            burst_count: 0,
            total_burst_ms: 0,
            ema_seeded: false,
        }
    }

    /// Create with custom frequency table
    pub fn with_freq_table(
        config: MinCutGovernorConfig,
        graph: ThermalGraph,
        actions: &[FreqAction],
    ) -> Self {
        let mut gov = Self::new(config, graph);
        let count = actions.len().min(MAX_FREQ_ACTIONS);
        for i in 0..count {
            gov.freq_actions[i] = actions[i];
        }
        gov.num_actions = count;
        gov.current_action = count / 3; // Start at ~lower third
        gov
    }

    /// Create pre-configured for Pi Zero 2W
    pub fn pi_zero_2w() -> Self {
        Self::new(MinCutGovernorConfig::default(), ThermalGraph::bcm2710a1())
    }

    /// Full update cycle: sensor input -> simulate -> mincut -> freq select
    ///
    /// # Arguments
    /// * `core_temps` - Temperature of each core [C0, C1, C2, C3]
    /// * `ambient_c` - Ambient temperature
    /// * `dt_ms` - Time delta in milliseconds
    ///
    /// # Returns
    /// Recommended frequency in MHz
    pub fn update(
        &mut self,
        core_temps: &[f32; 4],
        ambient_c: f32,
        dt_ms: u32,
    ) -> u32 {
        // 1. Update sensored node temperatures
        self.graph.update_temp(0, core_temps[0]);
        self.graph.update_temp(1, core_temps[1]);
        self.graph.update_temp(2, core_temps[2]);
        self.graph.update_temp(3, core_temps[3]);
        self.graph.update_temp(11, ambient_c);
        self.graph.update_temp(12, ambient_c);
        self.config.ambient_c = ambient_c;

        // 2. Simulate unmeasured node temperatures
        let dt_s = dt_ms as f32 / 1000.0;
        if dt_s > 0.0 {
            self.graph.simulate_step(dt_s);
        }

        // 3. Online calibration (gentle)
        if self.config.calibration_rate > 0.0 {
            self.graph.calibrate(self.config.calibration_rate);
        }

        // 4. Compute source-sink min-cut
        self.graph.invalidate();
        let mincut = self.graph.compute_mincut();

        // 5. EMA smoothing
        if !self.ema_seeded {
            self.mincut_ema = mincut;
            self.ema_seeded = true;
        } else {
            let a = self.config.ema_alpha;
            self.mincut_ema = (1.0 - a) * self.mincut_ema + a * mincut;
        }

        // 6. State machine
        self.state_time_ms += dt_ms;
        self.transition_state();

        // 7. Select frequency
        self.current_action = self.select_freq_action();
        self.freq_actions[self.current_action].freq_mhz
    }

    /// State machine transitions
    fn transition_state(&mut self) {
        match self.state {
            MinCutState::Throttle => {
                if self.mincut_ema >= self.config.baseline_threshold {
                    self.enter_state(MinCutState::Baseline);
                }
            }
            MinCutState::Baseline => {
                if self.mincut_ema < self.config.throttle_threshold {
                    self.enter_state(MinCutState::Throttle);
                } else if self.mincut_ema >= self.config.burst_threshold {
                    self.enter_state(MinCutState::Burst);
                    self.burst_count += 1;
                }
            }
            MinCutState::Burst => {
                if self.mincut_ema < self.config.baseline_threshold
                    || self.state_time_ms >= self.config.max_burst_ms
                {
                    self.total_burst_ms += self.state_time_ms as u64;
                    self.enter_state(MinCutState::Cooldown);
                }
            }
            MinCutState::Cooldown => {
                if self.state_time_ms >= self.config.cooldown_ms {
                    self.enter_state(MinCutState::Baseline);
                }
            }
        }
    }

    fn enter_state(&mut self, new_state: MinCutState) {
        self.state = new_state;
        self.state_time_ms = 0;
    }

    /// Select optimal frequency action for current state
    fn select_freq_action(&mut self) -> usize {
        let max_idx = self.num_actions - 1;

        match self.state {
            MinCutState::Throttle => 0,
            MinCutState::Baseline => {
                let range = self.config.burst_threshold - self.config.throttle_threshold;
                if range <= 0.0 {
                    return self.num_actions / 3;
                }
                let ratio = ((self.mincut_ema - self.config.throttle_threshold) / range)
                    .clamp(0.0, 1.0);
                // Map to lower-middle range
                let action = 1 + (ratio * (self.num_actions / 2) as f32) as usize;
                action.min(self.num_actions / 2)
            }
            MinCutState::Burst => {
                // Find highest frequency within thermal budget
                let max_q = self.graph.max_dissipation(
                    self.config.t_junction_max_c,
                    self.config.ambient_c,
                );
                let mut best = self.num_actions / 2; // Floor at mid-range
                for i in (best + 1..=max_idx).rev() {
                    if self.freq_actions[i].power_w <= max_q {
                        best = i;
                        break;
                    }
                }
                best
            }
            MinCutState::Cooldown => self.num_actions / 3,
        }
    }

    // --- Accessors ---

    pub fn state(&self) -> MinCutState { self.state }
    pub fn mincut_ema(&self) -> f32 { self.mincut_ema }
    pub fn current_freq_mhz(&self) -> u32 { self.freq_actions[self.current_action].freq_mhz }
    pub fn current_power_w(&self) -> f32 { self.freq_actions[self.current_action].power_w }
    pub fn burst_count(&self) -> u32 { self.burst_count }
    pub fn total_burst_ms(&self) -> u64 { self.total_burst_ms }
    pub fn graph(&self) -> &ThermalGraph { &self.graph }
    pub fn graph_mut(&mut self) -> &mut ThermalGraph { &mut self.graph }

    /// Manually trigger burst (if conditions allow)
    pub fn trigger_burst(&mut self) -> bool {
        if self.state == MinCutState::Baseline
            && self.mincut_ema >= self.config.baseline_threshold
        {
            self.enter_state(MinCutState::Burst);
            self.burst_count += 1;
            true
        } else {
            false
        }
    }

    /// Force immediate throttle
    pub fn force_throttle(&mut self) {
        self.enter_state(MinCutState::Throttle);
        self.current_action = 0;
    }

    /// Get max dissipation capacity in watts
    pub fn max_dissipation_w(&mut self) -> f32 {
        self.graph.max_dissipation(self.config.t_junction_max_c, self.config.ambient_c)
    }
}

// --- Burst Duty Cycle ---

/// Adaptive burst duty cycle pattern.
///
/// Generates burst/recovery oscillations and dynamically adjusts
/// the duty ratio based on thermal headroom feedback.
#[derive(Clone, Copy, Debug)]
pub struct BurstDutyCycle {
    /// Burst duration in milliseconds
    pub burst_ms: u32,
    /// Recovery duration in milliseconds
    pub recovery_ms: u32,
    /// Burst frequency in MHz
    pub burst_freq_mhz: u32,
    /// Recovery frequency in MHz
    pub recovery_freq_mhz: u32,
    /// Current position in the cycle (ms)
    position_ms: u32,
    /// Minimum burst ratio (floor)
    min_duty: f32,
    /// Maximum burst ratio (ceiling)
    max_duty: f32,
}

impl BurstDutyCycle {
    pub fn new(
        burst_ms: u32,
        recovery_ms: u32,
        burst_freq_mhz: u32,
        recovery_freq_mhz: u32,
    ) -> Self {
        Self {
            burst_ms,
            recovery_ms,
            burst_freq_mhz,
            recovery_freq_mhz,
            position_ms: 0,
            min_duty: 0.05,
            max_duty: 0.40,
        }
    }

    /// Pi Zero 2W with heatsink preset
    pub fn pi_zero_2w_heatsink() -> Self {
        Self::new(200, 800, 1500, 1000)
    }

    /// Pi Zero 2W stock (no heatsink) preset
    pub fn pi_zero_2w_stock() -> Self {
        Self::new(50, 950, 1300, 1000)
    }

    /// Advance cycle and return recommended frequency
    pub fn tick(&mut self, dt_ms: u32) -> u32 {
        self.position_ms += dt_ms;
        let period = self.burst_ms + self.recovery_ms;
        if period > 0 {
            self.position_ms %= period;
        }
        if self.position_ms < self.burst_ms {
            self.burst_freq_mhz
        } else {
            self.recovery_freq_mhz
        }
    }

    /// Adapt duty cycle based on thermal headroom ratio (0.0 = no headroom, 1.0 = max)
    pub fn adapt(&mut self, headroom_ratio: f32) {
        let target_duty = self.min_duty
            + (self.max_duty - self.min_duty) * headroom_ratio.clamp(0.0, 1.0);
        let total = self.burst_ms + self.recovery_ms;
        if total > 0 {
            self.burst_ms = (total as f32 * target_duty) as u32;
            self.recovery_ms = total - self.burst_ms;
            // Enforce minimums
            if self.burst_ms < 10 { self.burst_ms = 10; }
            if self.recovery_ms < 50 { self.recovery_ms = 50; }
        }
    }

    pub fn is_bursting(&self) -> bool { self.position_ms < self.burst_ms }

    pub fn duty_ratio(&self) -> f32 {
        let period = self.burst_ms + self.recovery_ms;
        if period > 0 { self.burst_ms as f32 / period as f32 } else { 0.0 }
    }

    pub fn effective_freq_mhz(&self) -> u32 {
        let r = self.duty_ratio();
        (r * self.burst_freq_mhz as f32 + (1.0 - r) * self.recovery_freq_mhz as f32) as u32
    }

    pub fn reset(&mut self) { self.position_ms = 0; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_graph_creation() {
        let g = ThermalGraph::new();
        assert_eq!(g.num_nodes(), 0);
        assert_eq!(g.num_edges(), 0);
    }

    #[test]
    fn test_bcm2710a1_graph() {
        let g = ThermalGraph::bcm2710a1();
        // 13 physical + 2 virtual = 15, but num_nodes tracks max index+1
        assert!(g.num_nodes() >= 13);
        assert!(g.num_edges() >= 14);
    }

    #[test]
    fn test_mincut_simple_series() {
        // source(14) -> 0 --0.5-- 1 --0.3-- 2 -> sink(15)
        let mut g = ThermalGraph::new();
        g.super_source = 14;
        g.super_sink = 15;
        g.add_node(0, 50.0, 0.01, ThermalNodeType::HeatSource);
        g.add_node(1, 35.0, 0.05, ThermalNodeType::Conductor);
        g.add_node(2, 25.0, f32::MAX, ThermalNodeType::HeatSink);
        g.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        g.add_edge(0, 1, 0.5);
        g.add_edge(1, 2, 0.3);
        g.add_edge(14, 0, 100.0); // super-source to source
        g.add_edge(2, 15, 100.0); // sink to super-sink
        g.rebuild_adjacency();

        let mincut = g.compute_mincut();
        assert!((mincut - 0.3).abs() < 0.05, "Expected ~0.3, got {}", mincut);
    }

    #[test]
    fn test_mincut_parallel_paths() {
        let mut g = ThermalGraph::new();
        g.super_source = 14;
        g.super_sink = 15;
        g.add_node(0, 50.0, 0.01, ThermalNodeType::HeatSource);
        g.add_node(1, 35.0, 0.05, ThermalNodeType::Conductor);
        g.add_node(2, 35.0, 0.05, ThermalNodeType::Conductor);
        g.add_node(3, 25.0, f32::MAX, ThermalNodeType::HeatSink);
        g.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        g.add_edge(0, 1, 10.0);
        g.add_edge(0, 2, 10.0);
        g.add_edge(1, 3, 0.5);
        g.add_edge(2, 3, 0.3);
        g.add_edge(14, 0, 100.0);
        g.add_edge(3, 15, 100.0);
        g.rebuild_adjacency();

        let mincut = g.compute_mincut();
        // Parallel paths: 0.5 + 0.3 = 0.8
        assert!((mincut - 0.8).abs() < 0.05, "Expected ~0.8, got {}", mincut);
    }

    #[test]
    fn test_bcm2710a1_mincut_positive() {
        let mut g = ThermalGraph::bcm2710a1();
        let mincut = g.compute_mincut();
        assert!(mincut > 0.0, "Min-cut should be positive, got {}", mincut);
        // Air convection total ~ 0.25+0.15+0.20 = 0.60 is the physical limit
        assert!(mincut < 10.0, "Min-cut unreasonably high: {}", mincut);
    }

    #[test]
    fn test_max_dissipation() {
        let mut g = ThermalGraph::new();
        g.super_source = 14;
        g.super_sink = 15;
        g.add_node(0, 50.0, 0.01, ThermalNodeType::HeatSource);
        g.add_node(1, 25.0, f32::MAX, ThermalNodeType::HeatSink);
        g.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        g.add_edge(0, 1, 0.3);
        g.add_edge(14, 0, 100.0);
        g.add_edge(1, 15, 100.0);
        g.rebuild_adjacency();

        // Q_max = 0.3 * (80 - 25) = 16.5W (approx, temp-dependent scaling)
        let max_q = g.max_dissipation(80.0, 25.0);
        assert!(max_q > 10.0 && max_q < 25.0, "Expected ~16.5W, got {}", max_q);
    }

    #[test]
    fn test_temp_dependent_conductance() {
        let mut g = ThermalGraph::new();
        g.super_source = 14;
        g.super_sink = 15;
        g.add_node(0, 25.0, 0.01, ThermalNodeType::HeatSource);
        g.add_node(1, 25.0, f32::MAX, ThermalNodeType::HeatSink);
        g.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        g.add_edge(0, 1, 1.0);
        g.add_edge(14, 0, 100.0);
        g.add_edge(1, 15, 100.0);
        g.rebuild_adjacency();

        let mc_cool = g.compute_mincut();

        // Heat up -> conductance drops
        g.update_temp(0, 80.0);
        g.invalidate();
        let mc_hot = g.compute_mincut();

        assert!(mc_hot < mc_cool,
            "Hot min-cut ({}) should be less than cool ({})", mc_hot, mc_cool);
    }

    #[test]
    fn test_thermal_simulation() {
        let mut g = ThermalGraph::new();
        g.add_node_full(0, 80.0, 0.05, 0.0, ThermalNodeType::Conductor, false);
        g.add_node_full(1, 25.0, f32::MAX, 0.0, ThermalNodeType::HeatSink, true);
        g.add_edge(0, 1, 1.0);
        g.rebuild_adjacency();

        let t_before = g.node_temp(0);
        g.simulate_step(0.1);
        let t_after = g.node_temp(0);

        // Node 0 should cool toward node 1 (25C)
        assert!(t_after < t_before,
            "Expected cooling: {} -> {}", t_before, t_after);
    }

    #[test]
    fn test_governor_creation() {
        let gov = MinCutGovernor::pi_zero_2w();
        assert_eq!(gov.state(), MinCutState::Baseline);
        assert_eq!(gov.burst_count(), 0);
    }

    #[test]
    fn test_governor_throttle_on_poor_thermal() {
        let config = MinCutGovernorConfig {
            throttle_threshold: 0.25,
            baseline_threshold: 0.35,
            burst_threshold: 0.50,
            calibration_rate: 0.0, // Disable calibration for test stability
            ..Default::default()
        };

        let mut graph = ThermalGraph::new();
        graph.super_source = 14;
        graph.super_sink = 15;
        graph.add_node(0, 70.0, 0.01, ThermalNodeType::HeatSource);
        graph.add_node(1, 25.0, f32::MAX, ThermalNodeType::HeatSink);
        graph.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        graph.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        graph.add_edge(0, 1, 0.1); // Poor thermal path
        graph.add_edge(14, 0, 100.0);
        graph.add_edge(1, 15, 100.0);
        graph.rebuild_adjacency();

        let mut gov = MinCutGovernor::new(config, graph);

        let temps = [70.0, 70.0, 70.0, 70.0];
        for _ in 0..10 {
            gov.update(&temps, 25.0, 100);
        }

        assert_eq!(gov.state(), MinCutState::Throttle);
        assert_eq!(gov.current_freq_mhz(), 600);
    }

    #[test]
    fn test_governor_burst_on_good_thermal() {
        let config = MinCutGovernorConfig {
            throttle_threshold: 0.05,
            baseline_threshold: 0.10,
            burst_threshold: 0.15,
            max_burst_ms: 200,
            cooldown_ms: 100,
            calibration_rate: 0.0,
            ..Default::default()
        };

        let mut graph = ThermalGraph::new();
        graph.super_source = 14;
        graph.super_sink = 15;
        graph.add_node(0, 40.0, 0.01, ThermalNodeType::HeatSource);
        graph.add_node(1, 25.0, f32::MAX, ThermalNodeType::HeatSink);
        graph.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        graph.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        graph.add_edge(0, 1, 1.0);
        graph.add_edge(14, 0, 100.0);
        graph.add_edge(1, 15, 100.0);
        graph.rebuild_adjacency();

        let mut gov = MinCutGovernor::new(config, graph);

        let temps = [40.0, 40.0, 40.0, 40.0];
        for _ in 0..20 {
            gov.update(&temps, 25.0, 10);
        }

        assert_eq!(gov.state(), MinCutState::Burst);
        assert!(gov.current_freq_mhz() >= 1300);
    }

    #[test]
    fn test_burst_duty_cycle() {
        let mut cycle = BurstDutyCycle::pi_zero_2w_heatsink();
        assert_eq!(cycle.burst_ms, 200);
        assert_eq!(cycle.recovery_ms, 800);

        let freq = cycle.tick(0);
        assert_eq!(freq, 1500);
        assert!(cycle.is_bursting());

        let freq = cycle.tick(250);
        assert_eq!(freq, 1000);
        assert!(!cycle.is_bursting());

        assert_eq!(cycle.effective_freq_mhz(), 1100);
    }

    #[test]
    fn test_duty_cycle_adapt() {
        let mut cycle = BurstDutyCycle::pi_zero_2w_heatsink();
        let total = cycle.burst_ms + cycle.recovery_ms;

        // Full headroom -> max duty
        cycle.adapt(1.0);
        assert!(cycle.duty_ratio() > 0.3);
        assert_eq!(cycle.burst_ms + cycle.recovery_ms, total);

        // No headroom -> min duty
        cycle.adapt(0.0);
        assert!(cycle.duty_ratio() < 0.1);
    }

    #[test]
    fn test_duty_cycle_wraps() {
        let mut cycle = BurstDutyCycle::new(100, 100, 1500, 1000);
        cycle.tick(150);
        assert!(!cycle.is_bursting());
        cycle.tick(100);
        assert!(cycle.is_bursting());
    }

    #[test]
    fn test_update_conductance() {
        let mut g = ThermalGraph::new();
        g.super_source = 14;
        g.super_sink = 15;
        g.add_node(0, 25.0, 0.01, ThermalNodeType::HeatSource);
        g.add_node(1, 25.0, f32::MAX, ThermalNodeType::HeatSink);
        g.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        g.add_edge(0, 1, 0.3);
        g.add_edge(14, 0, 100.0);
        g.add_edge(1, 15, 100.0);
        g.rebuild_adjacency();

        let mc1 = g.compute_mincut();

        g.update_conductance(0, 1, 0.6);
        let mc2 = g.compute_mincut();
        assert!(mc2 > mc1, "Doubled conductance should increase min-cut");
    }

    #[test]
    fn test_incremental_skip() {
        let mut g = ThermalGraph::new();
        g.super_source = 14;
        g.super_sink = 15;
        g.add_node_full(0, 50.0, 0.01, 0.0, ThermalNodeType::HeatSource, true);
        g.add_node_full(1, 25.0, f32::MAX, 0.0, ThermalNodeType::HeatSink, true);
        g.add_node_full(14, 0.0, 0.0, 0.0, ThermalNodeType::HeatSource, false);
        g.add_node_full(15, 0.0, 0.0, 0.0, ThermalNodeType::HeatSink, false);
        g.add_edge(0, 1, 0.5);
        g.add_edge(14, 0, 100.0);
        g.add_edge(1, 15, 100.0);
        g.rebuild_adjacency();

        let _mc = g.compute_mincut();
        assert!(g.cache_valid);

        // Tiny temp change -> should NOT recompute
        g.update_temp(0, 50.1);
        assert!(!g.needs_recompute());

        // Large temp change -> should recompute
        g.update_temp(0, 55.0);
        assert!(g.needs_recompute());
    }
}
