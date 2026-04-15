//! Test fixture generators

use serde::{Deserialize, Serialize};

/// Generate test user fixture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestUser {
    pub id: String,
    pub email: String,
    pub tier: String,
}

impl TestUser {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            email: format!("{}@example.com", id),
            tier: "professional".to_string(),
        }
    }

    pub fn with_tier(mut self, tier: &str) -> Self {
        self.tier = tier.to_string();
        self
    }
}

/// Generate test simulation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSimulationRequest {
    pub program: Vec<u8>,
    pub max_cycles: u64,
    pub tiles: u32,
}

impl TestSimulationRequest {
    pub fn default() -> Self {
        Self {
            program: vec![0x01, 0x02, 0x03, 0x04],
            max_cycles: 10000,
            tiles: 64,
        }
    }

    pub fn with_program(mut self, program: Vec<u8>) -> Self {
        self.program = program;
        self
    }

    pub fn with_cycles(mut self, cycles: u64) -> Self {
        self.max_cycles = cycles;
        self
    }
}

/// Generate test API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> TestApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.to_string()),
        }
    }
}

/// Generate test license
#[derive(Debug, Clone)]
pub struct TestLicense {
    pub key: String,
    pub tier: String,
    pub expires_at: i64,
}

impl TestLicense {
    pub fn valid() -> Self {
        Self {
            key: "lic_test_valid".to_string(),
            tier: "professional".to_string(),
            expires_at: i64::MAX,
        }
    }

    pub fn expired() -> Self {
        Self {
            key: "lic_test_expired".to_string(),
            tier: "professional".to_string(),
            expires_at: 0,
        }
    }
}

/// Builder for test vectors
pub struct TestVectorBuilder {
    dimensions: usize,
    values: Option<Vec<f32>>,
}

impl TestVectorBuilder {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            values: None,
        }
    }

    pub fn with_values(mut self, values: Vec<f32>) -> Self {
        self.values = Some(values);
        self
    }

    pub fn random(self) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..self.dimensions).map(|_| rng.gen()).collect()
    }

    pub fn zeros(self) -> Vec<f32> {
        vec![0.0; self.dimensions]
    }

    pub fn ones(self) -> Vec<f32> {
        vec![1.0; self.dimensions]
    }

    pub fn build(self) -> Vec<f32> {
        self.values.unwrap_or_else(|| vec![0.0; self.dimensions])
    }
}
