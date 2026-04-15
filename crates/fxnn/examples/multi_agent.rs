//! Multi-Agent Emergence Example
//!
//! This example demonstrates the Emergence layer (Layer 5) of FXNN's Five-Layer
//! Reality Stack, where collective behaviors arise from individual agent
//! interactions without central coordination.
//!
//! # Concept
//!
//! Multiple agents follow simple local rules, yet exhibit complex collective
//! phenomena such as:
//!
//! - **Flocking**: Agents align velocities and cluster together
//! - **Separation**: Agents avoid collisions with neighbors
//! - **Cohesion**: Agents move toward the center of local group
//!
//! These are the famous "Boids" rules that produce flocking behavior in birds,
//! fish schools, and other collective animal behaviors.
//!
//! # Reality Stack Layers
//!
//! 1. **Substrate**: Agent particles in 2D periodic box
//! 2. **Forces**: Inter-agent attraction/repulsion
//! 3. **Dynamics**: Euler integration with velocity limiting
//! 4. **Agency**: Each agent perceives neighbors and decides action
//! 5. **Emergence**: Collective patterns arise from local interactions
//!
//! # Emergent Behaviors to Observe
//!
//! - Formation of stable clusters
//! - Alignment of movement direction
//! - Dynamic splitting and merging of groups
//! - Edge effects and boundary interactions
//! - Spontaneous symmetry breaking
//!
//! # Running
//!
//! ```bash
//! cargo run --example multi_agent --release
//! ```
//!
//! # Metrics Computed
//!
//! - **Order Parameter**: Average alignment of velocities (0 = disorder, 1 = order)
//! - **Cluster Count**: Number of distinct groups
//! - **Cohesion**: Average distance to group center
//! - **Polarization**: Net direction of movement
//!
//! # Extending
//!
//! Try modifying:
//! - Number of agents (observe phase transitions)
//! - Interaction radii (perception range)
//! - Rule weights (alignment, cohesion, separation)
//! - Add predator-prey dynamics
//! - Add environmental features (food sources, obstacles)

use std::f32::consts::PI;

/// Boid parameters
mod boid {
    /// Mass of each agent
    pub const MASS: f32 = 1.0;

    /// Maximum speed
    pub const MAX_SPEED: f32 = 2.0;

    /// Maximum force (steering ability)
    pub const MAX_FORCE: f32 = 0.5;

    /// Perception radius for neighbors
    pub const PERCEPTION_RADIUS: f32 = 3.0;

    /// Separation distance (avoid collisions)
    pub const SEPARATION_DIST: f32 = 1.0;

    /// Alignment weight
    pub const ALIGNMENT_WEIGHT: f32 = 1.0;

    /// Cohesion weight
    pub const COHESION_WEIGHT: f32 = 0.8;

    /// Separation weight
    pub const SEPARATION_WEIGHT: f32 = 1.5;

    /// Boundary avoidance weight
    pub const BOUNDARY_WEIGHT: f32 = 2.0;
}

/// A single Boid agent
#[derive(Clone)]
struct Boid {
    id: usize,
    position: [f32; 2],
    velocity: [f32; 2],
    acceleration: [f32; 2],
}

impl Boid {
    fn new(id: usize, x: f32, y: f32, vx: f32, vy: f32) -> Self {
        Self {
            id,
            position: [x, y],
            velocity: [vx, vy],
            acceleration: [0.0, 0.0],
        }
    }

    /// Apply steering force
    fn apply_force(&mut self, fx: f32, fy: f32) {
        self.acceleration[0] += fx / boid::MASS;
        self.acceleration[1] += fy / boid::MASS;
    }

    /// Update position and velocity
    fn update(&mut self, dt: f32, box_size: f32) {
        // Update velocity
        self.velocity[0] += self.acceleration[0] * dt;
        self.velocity[1] += self.acceleration[1] * dt;

        // Limit speed
        let speed = (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt();
        if speed > boid::MAX_SPEED {
            let scale = boid::MAX_SPEED / speed;
            self.velocity[0] *= scale;
            self.velocity[1] *= scale;
        }

        // Update position
        self.position[0] += self.velocity[0] * dt;
        self.position[1] += self.velocity[1] * dt;

        // Wrap around boundaries (periodic)
        if self.position[0] < 0.0 { self.position[0] += box_size; }
        if self.position[0] >= box_size { self.position[0] -= box_size; }
        if self.position[1] < 0.0 { self.position[1] += box_size; }
        if self.position[1] >= box_size { self.position[1] -= box_size; }

        // Reset acceleration
        self.acceleration = [0.0, 0.0];
    }

    /// Get speed
    fn speed(&self) -> f32 {
        (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt()
    }

    /// Get heading angle
    fn heading(&self) -> f32 {
        self.velocity[1].atan2(self.velocity[0])
    }
}

/// The flock of boids
struct Flock {
    boids: Vec<Boid>,
    box_size: f32,
}

impl Flock {
    fn new(n_boids: usize, box_size: f32) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let boids = (0..n_boids)
            .map(|id| {
                let x = rng.gen::<f32>() * box_size;
                let y = rng.gen::<f32>() * box_size;
                let angle = rng.gen::<f32>() * 2.0 * PI;
                let speed = rng.gen::<f32>() * boid::MAX_SPEED;
                Boid::new(id, x, y, speed * angle.cos(), speed * angle.sin())
            })
            .collect();

        Self { boids, box_size }
    }

    /// Calculate distance with periodic boundaries
    fn distance(&self, a: &Boid, b: &Boid) -> f32 {
        let mut dx = b.position[0] - a.position[0];
        let mut dy = b.position[1] - a.position[1];

        // Minimum image convention
        if dx > self.box_size / 2.0 { dx -= self.box_size; }
        if dx < -self.box_size / 2.0 { dx += self.box_size; }
        if dy > self.box_size / 2.0 { dy -= self.box_size; }
        if dy < -self.box_size / 2.0 { dy += self.box_size; }

        (dx * dx + dy * dy).sqrt()
    }

    /// Vector from a to b with periodic boundaries
    fn displacement(&self, from: &Boid, to: &Boid) -> [f32; 2] {
        let mut dx = to.position[0] - from.position[0];
        let mut dy = to.position[1] - from.position[1];

        if dx > self.box_size / 2.0 { dx -= self.box_size; }
        if dx < -self.box_size / 2.0 { dx += self.box_size; }
        if dy > self.box_size / 2.0 { dy -= self.box_size; }
        if dy < -self.box_size / 2.0 { dy += self.box_size; }

        [dx, dy]
    }

    /// Calculate flocking forces for all boids
    fn calculate_forces(&mut self) {
        let n = self.boids.len();

        for i in 0..n {
            let mut alignment = [0.0f32; 2];
            let mut cohesion = [0.0f32; 2];
            let mut separation = [0.0f32; 2];
            let mut n_neighbors = 0;

            for j in 0..n {
                if i == j { continue; }

                let dist = self.distance(&self.boids[i], &self.boids[j]);

                if dist < boid::PERCEPTION_RADIUS {
                    // Alignment: match neighbor velocities
                    alignment[0] += self.boids[j].velocity[0];
                    alignment[1] += self.boids[j].velocity[1];

                    // Cohesion: steer toward center of neighbors
                    let d = self.displacement(&self.boids[i], &self.boids[j]);
                    cohesion[0] += d[0];
                    cohesion[1] += d[1];

                    // Separation: avoid close neighbors
                    if dist < boid::SEPARATION_DIST && dist > 0.01 {
                        separation[0] -= d[0] / dist;
                        separation[1] -= d[1] / dist;
                    }

                    n_neighbors += 1;
                }
            }

            if n_neighbors > 0 {
                let n = n_neighbors as f32;

                // Average and apply weights
                alignment[0] = alignment[0] / n * boid::ALIGNMENT_WEIGHT;
                alignment[1] = alignment[1] / n * boid::ALIGNMENT_WEIGHT;

                cohesion[0] = cohesion[0] / n * boid::COHESION_WEIGHT;
                cohesion[1] = cohesion[1] / n * boid::COHESION_WEIGHT;

                separation[0] *= boid::SEPARATION_WEIGHT;
                separation[1] *= boid::SEPARATION_WEIGHT;

                // Combine forces
                let mut fx = alignment[0] + cohesion[0] + separation[0];
                let mut fy = alignment[1] + cohesion[1] + separation[1];

                // Limit force magnitude
                let mag = (fx * fx + fy * fy).sqrt();
                if mag > boid::MAX_FORCE {
                    let scale = boid::MAX_FORCE / mag;
                    fx *= scale;
                    fy *= scale;
                }

                self.boids[i].apply_force(fx, fy);
            }
        }
    }

    /// Update all boids
    fn update(&mut self, dt: f32) {
        for boid in &mut self.boids {
            boid.update(dt, self.box_size);
        }
    }

    /// Step the simulation
    fn step(&mut self, dt: f32) {
        self.calculate_forces();
        self.update(dt);
    }

    // =========================================================================
    // Emergence Metrics
    // =========================================================================

    /// Order parameter: average alignment of velocities
    /// 1.0 = all aligned, 0.0 = random
    fn order_parameter(&self) -> f32 {
        let mut sum_vx = 0.0f32;
        let mut sum_vy = 0.0f32;
        let mut sum_speed = 0.0f32;

        for boid in &self.boids {
            sum_vx += boid.velocity[0];
            sum_vy += boid.velocity[1];
            sum_speed += boid.speed();
        }

        let net_velocity = (sum_vx.powi(2) + sum_vy.powi(2)).sqrt();
        if sum_speed > 0.01 {
            net_velocity / sum_speed
        } else {
            0.0
        }
    }

    /// Average speed
    fn average_speed(&self) -> f32 {
        let total: f32 = self.boids.iter().map(|b| b.speed()).sum();
        total / self.boids.len() as f32
    }

    /// Center of mass
    fn center_of_mass(&self) -> [f32; 2] {
        let n = self.boids.len() as f32;
        let cx: f32 = self.boids.iter().map(|b| b.position[0]).sum::<f32>() / n;
        let cy: f32 = self.boids.iter().map(|b| b.position[1]).sum::<f32>() / n;
        [cx, cy]
    }

    /// Average distance to center of mass (cohesion metric)
    fn cohesion_metric(&self) -> f32 {
        let com = self.center_of_mass();
        let total_dist: f32 = self.boids.iter()
            .map(|b| {
                let dx = b.position[0] - com[0];
                let dy = b.position[1] - com[1];
                (dx * dx + dy * dy).sqrt()
            })
            .sum();
        total_dist / self.boids.len() as f32
    }

    /// Count clusters using simple distance threshold
    fn count_clusters(&self, threshold: f32) -> usize {
        let n = self.boids.len();
        let mut visited = vec![false; n];
        let mut clusters = 0;

        for i in 0..n {
            if visited[i] { continue; }

            // BFS to find connected component
            let mut stack = vec![i];
            while let Some(curr) = stack.pop() {
                if visited[curr] { continue; }
                visited[curr] = true;

                for j in 0..n {
                    if !visited[j] && self.distance(&self.boids[curr], &self.boids[j]) < threshold {
                        stack.push(j);
                    }
                }
            }
            clusters += 1;
        }

        clusters
    }

    /// Polarization: net direction of movement
    fn polarization(&self) -> f32 {
        let mut sum_heading_x = 0.0f32;
        let mut sum_heading_y = 0.0f32;

        for boid in &self.boids {
            let heading = boid.heading();
            sum_heading_x += heading.cos();
            sum_heading_y += heading.sin();
        }

        let n = self.boids.len() as f32;
        (sum_heading_x.powi(2) + sum_heading_y.powi(2)).sqrt() / n
    }
}

fn main() {
    println!("=======================================================");
    println!("  FXNN Multi-Agent Emergence Simulation");
    println!("  Layer 5: Collective Intelligence");
    println!("=======================================================\n");

    // =========================================================================
    // Configuration
    // =========================================================================

    println!("[Configuration]");

    let n_boids = 50;
    let box_size = 20.0;
    let dt = 0.1;
    let total_steps = 5000;
    let report_interval = 500;

    println!("  Number of agents: {}", n_boids);
    println!("  Arena size: {:.1} x {:.1}", box_size, box_size);
    println!("  Timestep: {:.2}", dt);
    println!("  Total steps: {}", total_steps);

    // =========================================================================
    // Layer Setup
    // =========================================================================

    println!("\n[Five-Layer Reality Stack]");
    println!("  L1 Substrate:  {} particles in 2D periodic box", n_boids);
    println!("  L2 Forces:     Alignment + Cohesion + Separation");
    println!("  L3 Dynamics:   Euler integration, speed-limited");
    println!("  L4 Agency:     Local perception ({:.1} radius), rule-based policy",
             boid::PERCEPTION_RADIUS);
    println!("  L5 Emergence:  Flocking behavior from simple rules");

    // =========================================================================
    // Initialize Flock
    // =========================================================================

    println!("\n[Initialization]");

    let mut flock = Flock::new(n_boids, box_size);

    println!("  Random initial positions and velocities");
    println!("  Initial order parameter: {:.3}", flock.order_parameter());
    println!("  Initial clusters: {}", flock.count_clusters(boid::PERCEPTION_RADIUS));

    // =========================================================================
    // Simulation
    // =========================================================================

    println!("\n[Emergence Simulation]");
    println!("  Observing collective behavior...\n");

    println!("  {:>6} {:>8} {:>10} {:>10} {:>8} {:>10}",
             "Step", "Order", "Cohesion", "Polar", "Clusters", "Avg Speed");
    println!("  {}", "-".repeat(66));

    let mut order_history = Vec::new();

    for step in 0..=total_steps {
        if step % report_interval == 0 {
            let order = flock.order_parameter();
            let cohesion = flock.cohesion_metric();
            let polar = flock.polarization();
            let clusters = flock.count_clusters(boid::PERCEPTION_RADIUS);
            let speed = flock.average_speed();

            order_history.push(order);

            println!("  {:>6} {:>8.3} {:>10.3} {:>10.3} {:>8} {:>10.3}",
                     step, order, cohesion, polar, clusters, speed);
        }

        if step < total_steps {
            flock.step(dt);
        }
    }

    // =========================================================================
    // Analysis
    // =========================================================================

    println!("\n[Emergence Analysis]");
    println!("  --------------------------------------------------");

    let initial_order = order_history.first().unwrap_or(&0.0);
    let final_order = order_history.last().unwrap_or(&0.0);
    let avg_order: f32 = order_history.iter().sum::<f32>() / order_history.len() as f32;

    println!("  Initial order parameter:  {:>10.4}", initial_order);
    println!("  Final order parameter:    {:>10.4}", final_order);
    println!("  Average order parameter:  {:>10.4}", avg_order);
    println!("  Final cluster count:      {:>10}", flock.count_clusters(boid::PERCEPTION_RADIUS));
    println!("  Final cohesion:           {:>10.4}", flock.cohesion_metric());
    println!("  Final polarization:       {:>10.4}", flock.polarization());
    println!("  --------------------------------------------------");

    // =========================================================================
    // Phase Classification
    // =========================================================================

    println!("\n[Phase Classification]");

    if *final_order > 0.8 {
        println!("  System phase: ORDERED (high alignment)");
        println!("  ");
        println!("  The flock has achieved coordinated movement.");
        println!("  Most agents are moving in the same direction.");
        println!("  This represents a collective \"decision\" emerging");
        println!("  from purely local interactions.");
    } else if *final_order > 0.4 {
        println!("  System phase: TRANSITIONAL (partial order)");
        println!("  ");
        println!("  The system shows partial coordination.");
        println!("  Multiple sub-flocks may exist with different directions.");
        println!("  This is near the phase transition between order and disorder.");
    } else {
        println!("  System phase: DISORDERED (low alignment)");
        println!("  ");
        println!("  Agents are moving in random directions.");
        println!("  No global coordination has emerged.");
        println!("  Try increasing perception radius or reducing noise.");
    }

    let cluster_count = flock.count_clusters(boid::PERCEPTION_RADIUS);
    println!("  ");
    if cluster_count == 1 {
        println!("  Cluster state: UNIFIED (single flock)");
        println!("  All agents belong to one connected group.");
    } else if cluster_count <= 3 {
        println!("  Cluster state: FEW GROUPS ({} flocks)", cluster_count);
        println!("  Multiple stable sub-flocks have formed.");
    } else {
        println!("  Cluster state: FRAGMENTED ({} groups)", cluster_count);
        println!("  Agents are distributed in many small groups.");
    }

    // =========================================================================
    // Emergence Metrics Summary
    // =========================================================================

    println!("\n[What Emerged]");
    println!("  ");
    println!("  From simple local rules:");
    println!("    - Align with neighbors");
    println!("    - Move toward neighbor center");
    println!("    - Avoid collisions");
    println!("  ");
    println!("  Complex behaviors arose:");
    if *final_order > 0.5 {
        println!("    [x] Coordinated movement");
    } else {
        println!("    [ ] Coordinated movement");
    }
    if cluster_count <= 3 {
        println!("    [x] Group formation");
    } else {
        println!("    [ ] Group formation");
    }
    if flock.cohesion_metric() < box_size / 4.0 {
        println!("    [x] Spatial clustering");
    } else {
        println!("    [ ] Spatial clustering");
    }
    if flock.polarization() > 0.5 {
        println!("    [x] Collective direction");
    } else {
        println!("    [ ] Collective direction");
    }
    println!("  ");
    println!("  No central controller directed this behavior.");
    println!("  It emerged from the interactions at Layer 5.");

    println!("\n=======================================================");
    println!("  Multi-Agent Emergence Complete");
    println!("  FXNN - Where Physics Meets Intelligence");
    println!("=======================================================\n");
}
