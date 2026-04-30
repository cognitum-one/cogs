//! Cognitum Cog: GOAP Autonomy
//!
//! Goal-Oriented Action Planning. Define goals (e.g., "maximize coverage")
//! and actions (e.g., "adjust sensitivity", "switch channels"). Plan action
//! sequences to achieve goals.
//!
//! Usage:
//!   cog-goap-autonomy --once
//!   cog-goap-autonomy --interval 10

use std::io::Read;
use std::time::{Duration, Instant};

const DIM: usize = 8;

/// World state as a set of key-value properties
type WorldState = std::collections::HashMap<String, f64>;

/// An action the system can take
struct Action {
    name: String,
    /// Preconditions: required state values (key, min_value)
    preconditions: Vec<(String, f64)>,
    /// Effects: state changes (key, delta)
    effects: Vec<(String, f64)>,
    /// Cost of performing this action
    cost: f64,
}

impl Action {
    fn is_applicable(&self, state: &WorldState) -> bool {
        self.preconditions.iter().all(|(key, min_val)| {
            state.get(key).map(|v| *v >= *min_val).unwrap_or(false)
        })
    }

    fn apply(&self, state: &mut WorldState) {
        for (key, delta) in &self.effects {
            let entry = state.entry(key.clone()).or_insert(0.0);
            *entry += delta;
        }
    }
}

/// A goal with target state values
struct Goal {
    name: String,
    /// Required state (key, min_value)
    requirements: Vec<(String, f64)>,
    /// Priority (higher = more important)
    priority: f64,
}

impl Goal {
    fn is_satisfied(&self, state: &WorldState) -> bool {
        self.requirements.iter().all(|(key, min_val)| {
            state.get(key).map(|v| *v >= *min_val).unwrap_or(false)
        })
    }

    fn satisfaction_score(&self, state: &WorldState) -> f64 {
        if self.requirements.is_empty() { return 1.0; }
        let scores: Vec<f64> = self.requirements.iter().map(|(key, target)| {
            let current = state.get(key).cloned().unwrap_or(0.0);
            if *target <= 0.0 { return 1.0; }
            (current / target).min(1.0).max(0.0)
        }).collect();
        scores.iter().sum::<f64>() / scores.len() as f64
    }
}

/// Simple forward planner (BFS with max depth)
fn plan(state: &WorldState, goal: &Goal, actions: &[Action], max_depth: usize) -> Vec<String> {
    if goal.is_satisfied(state) {
        return Vec::new();
    }

    // BFS
    struct Node {
        state: WorldState,
        actions: Vec<String>,
        cost: f64,
    }

    let mut queue = std::collections::VecDeque::new();
    queue.push_back(Node {
        state: state.clone(),
        actions: Vec::new(),
        cost: 0.0,
    });

    let mut best_plan: Option<(Vec<String>, f64)> = None;

    while let Some(node) = queue.pop_front() {
        if node.actions.len() >= max_depth {
            continue;
        }

        for action in actions {
            if action.is_applicable(&node.state) {
                let mut new_state = node.state.clone();
                action.apply(&mut new_state);
                let mut new_actions = node.actions.clone();
                new_actions.push(action.name.clone());
                let new_cost = node.cost + action.cost;

                if goal.is_satisfied(&new_state) {
                    if best_plan.as_ref().map(|(_, c)| new_cost < *c).unwrap_or(true) {
                        best_plan = Some((new_actions.clone(), new_cost));
                    }
                }

                if new_actions.len() < max_depth {
                    queue.push_back(Node {
                        state: new_state,
                        actions: new_actions,
                        cost: new_cost,
                    });
                }
            }
        }
    }

    best_plan.map(|(p, _)| p).unwrap_or_default()
}

#[derive(serde::Serialize)]
struct GoapResult {
    current_state: std::collections::HashMap<String, f64>,
    active_goal: String,
    goal_satisfaction: f64,
    planned_actions: Vec<String>,
    plan_cost: f64,
    goals_evaluated: usize,
    vector: [f64; DIM],
    timestamp: u64,
}

fn fetch_sensors() -> Result<serde_json::Value, String> {
    cog_sensor_sources::fetch_sensors()
}

fn store_vector(v: &[f64; DIM]) -> Result<(), String> {
    let payload = serde_json::json!({ "vectors": [[0, v]], "dedup": true });
    let body = serde_json::to_vec(&payload).map_err(|e| format!("json: {e}"))?;
    let mut conn = std::net::TcpStream::connect("127.0.0.1:80")
        .map_err(|e| format!("connect: {e}"))?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    conn.set_write_timeout(Some(std::time::Duration::from_secs(5))).ok();
    use std::io::Write;
    write!(conn, "POST /api/v1/store/ingest HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len())
        .map_err(|e| format!("write: {e}"))?;
    conn.write_all(&body).map_err(|e| format!("write body: {e}"))?;
    let mut resp = Vec::new();
    conn.read_to_end(&mut resp).ok();
    Ok(())
}

fn run_once() -> Result<GoapResult, String> {
    let sensors = fetch_sensors()?;
    let samples = sensors.get("samples").and_then(|s| s.as_array()).ok_or("no samples")?;

    // Build world state from sensor data
    let mut state = WorldState::new();
    let mut values: Vec<f64> = Vec::new();
    for s in samples {
        let ch = s.get("channel").and_then(|c| c.as_str()).unwrap_or("ch0");
        let val = s.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        state.insert(format!("{ch}_level"), val);
        values.push(val);
    }

    let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
    let active_channels = values.iter().filter(|&&v| v.abs() > 0.01).count();
    state.insert("coverage".into(), active_channels as f64 / values.len().max(1) as f64);
    state.insert("sensitivity".into(), 0.5);
    state.insert("signal_quality".into(), mean.abs().min(1.0));
    state.insert("energy".into(), 1.0);

    // Define actions
    let actions = vec![
        Action {
            name: "increase_sensitivity".into(),
            preconditions: vec![("energy".into(), 0.2)],
            effects: vec![("sensitivity".into(), 0.1), ("coverage".into(), 0.1), ("energy".into(), -0.1)],
            cost: 1.0,
        },
        Action {
            name: "decrease_sensitivity".into(),
            preconditions: vec![("sensitivity".into(), 0.2)],
            effects: vec![("sensitivity".into(), -0.1), ("energy".into(), 0.05)],
            cost: 0.5,
        },
        Action {
            name: "activate_channel".into(),
            preconditions: vec![("energy".into(), 0.3)],
            effects: vec![("coverage".into(), 0.15), ("energy".into(), -0.15)],
            cost: 1.5,
        },
        Action {
            name: "calibrate".into(),
            preconditions: vec![("energy".into(), 0.1)],
            effects: vec![("signal_quality".into(), 0.2), ("energy".into(), -0.05)],
            cost: 0.8,
        },
        Action {
            name: "rest".into(),
            preconditions: vec![],
            effects: vec![("energy".into(), 0.3)],
            cost: 2.0,
        },
    ];

    // Define goals
    let goals = vec![
        Goal {
            name: "maximize_coverage".into(),
            requirements: vec![("coverage".into(), 0.8)],
            priority: 1.0,
        },
        Goal {
            name: "maintain_quality".into(),
            requirements: vec![("signal_quality".into(), 0.7)],
            priority: 0.8,
        },
        Goal {
            name: "conserve_energy".into(),
            requirements: vec![("energy".into(), 0.5)],
            priority: 0.6,
        },
    ];

    // Find highest-priority unsatisfied goal
    let active_goal = goals.iter()
        .filter(|g| !g.is_satisfied(&state))
        .max_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap_or(std::cmp::Ordering::Equal));

    let (goal_name, satisfaction, planned) = if let Some(goal) = active_goal {
        let p = plan(&state, goal, &actions, 5);
        (goal.name.clone(), goal.satisfaction_score(&state), p)
    } else {
        ("all_satisfied".into(), 1.0, Vec::new())
    };

    let plan_cost: f64 = planned.iter().map(|name| {
        actions.iter().find(|a| a.name == *name).map(|a| a.cost).unwrap_or(0.0)
    }).sum();

    let vector = [
        satisfaction,
        plan_cost / 10.0,
        planned.len() as f64 / 5.0,
        state.get("coverage").cloned().unwrap_or(0.0),
        state.get("sensitivity").cloned().unwrap_or(0.0),
        state.get("signal_quality").cloned().unwrap_or(0.0),
        state.get("energy").cloned().unwrap_or(0.0),
        goals.len() as f64 / 10.0,
    ];

    let _ = store_vector(&vector);

    Ok(GoapResult {
        current_state: state,
        active_goal: goal_name,
        goal_satisfaction: satisfaction,
        planned_actions: planned,
        plan_cost,
        goals_evaluated: goals.len(),
        vector,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let once = args.iter().any(|a| a == "--once");
    let interval = args.iter().position(|a| a == "--interval")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    eprintln!("[cog-goap-autonomy] starting (interval={interval}s, once={once})");

    loop {
        let start = Instant::now();
        match run_once() {
            Ok(r) => {
                println!("{}", serde_json::to_string(&r).unwrap_or_default());
                if r.goal_satisfaction < 0.5 {
                    eprintln!("[cog-goap-autonomy] ALERT: low goal satisfaction ({:.2}), plan: {:?}", r.goal_satisfaction, r.planned_actions);
                }
            }
            Err(e) => eprintln!("[cog-goap-autonomy] error: {e}"),
        }
        if once { break; }
        let elapsed = start.elapsed();
        if elapsed < Duration::from_secs(interval) {
            std::thread::sleep(Duration::from_secs(interval) - elapsed);
        }
    }
}
