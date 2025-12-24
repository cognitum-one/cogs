//! HTTP Request Handlers for Cognitum API
//!
//! This module implements handler functions for all API endpoints.
//! Handlers are organized by domain:
//! - `auth_handlers`: Authentication and API key management
//! - `simulator_handlers`: Simulation execution and status
//! - `ruvector_handlers`: Vector search operations

use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::RwLock;
use argon2::{Argon2, PasswordHash, PasswordVerifier, PasswordHasher};
use argon2::password_hash::{SaltString, rand_core::OsRng};

use crate::api::{InMemoryStore, RateLimitConfig};
use crate::auth::{ApiKeyService, JwtService, KeyScope};
use crate::auth::types::{UserId, UserClaims};
use crate::sdk::core::CognitumSimulator;
use crate::ruvector::facade::CognitumRuvector;

/// User record for authentication
#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub roles: Vec<String>,
    pub tier: String,
}

/// Simple in-memory user store
pub struct UserStore {
    users: RwLock<HashMap<String, User>>,
}

impl UserStore {
    pub fn new() -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_user(&self, username: &str, password_hash: &str, tier: &str) -> Result<User, String> {
        let mut users = self.users.write();

        if users.contains_key(username) {
            return Err("User already exists".to_string());
        }

        let user = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            roles: vec!["user".to_string()],
            tier: tier.to_string(),
        };

        users.insert(username.to_string(), user.clone());
        Ok(user)
    }

    pub fn get_user_by_username(&self, username: &str) -> Option<User> {
        let users = self.users.read();
        users.get(username).cloned()
    }
}

/// Authenticated user extracted from middleware
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub roles: Vec<String>,
}

/// Shared application state passed to all handlers
#[derive(Clone)]
pub struct ApiState {
    pub rate_limit_store: Arc<InMemoryStore>,
    pub rate_limit_config: RateLimitConfig,
    pub jwt_service: Arc<JwtService>,
    pub api_key_service: Arc<ApiKeyService>,
    pub user_store: Arc<UserStore>,
    pub simulator: Arc<tokio::sync::Mutex<CognitumSimulator>>,
    pub ruvector: Arc<CognitumRuvector>,
}

/// Standard API error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Standard API success response
#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessResponse<T> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl<T> SuccessResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data, meta: None }
    }

    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = Some(meta);
        self
    }
}

/// Convert errors to HTTP responses
impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.code.as_str() {
            "INVALID_CREDENTIALS" => StatusCode::UNAUTHORIZED,
            "TOKEN_EXPIRED" => StatusCode::UNAUTHORIZED,
            "INVALID_TOKEN" => StatusCode::UNAUTHORIZED,
            "KEY_REVOKED" => StatusCode::FORBIDDEN,
            "RATE_LIMIT_EXCEEDED" => StatusCode::TOO_MANY_REQUESTS,
            "INVALID_REQUEST" => StatusCode::BAD_REQUEST,
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self)).into_response()
    }
}

/// Authentication handlers module
pub mod auth_handlers {
    use super::*;

    /// Login request body
    #[derive(Debug, Deserialize)]
    pub struct LoginRequest {
        pub username: String,
        pub password: String,
    }

    /// Login response with tokens
    #[derive(Debug, Serialize)]
    pub struct LoginResponse {
        pub access_token: String,
        pub refresh_token: String,
        pub token_type: String,
        pub expires_in: i64,
    }

    /// Refresh token request
    #[derive(Debug, Deserialize)]
    pub struct RefreshTokenRequest {
        pub refresh_token: String,
    }

    /// Register request body
    #[derive(Debug, Deserialize)]
    pub struct RegisterRequest {
        pub username: String,
        pub password: String,
        pub tier: Option<String>,
    }

    /// Register response
    #[derive(Debug, Serialize)]
    pub struct RegisterResponse {
        pub user_id: String,
        pub username: String,
    }

    /// Create API key request
    #[derive(Debug, Deserialize)]
    pub struct CreateApiKeyRequest {
        pub scope: KeyScope,
        pub description: Option<String>,
    }

    /// Create API key response
    #[derive(Debug, Serialize)]
    pub struct CreateApiKeyResponse {
        pub api_key: String,
        pub key_id: String,
        pub scope: KeyScope,
        pub created_at: chrono::DateTime<chrono::Utc>,
    }

    /// Login endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// post:
    ///   summary: Authenticate user and get JWT tokens
    ///   tags: [Authentication]
    ///   requestBody:
    ///     required: true
    ///     content:
    ///       application/json:
    ///         schema:
    ///           type: object
    ///           required: [username, password]
    ///           properties:
    ///             username:
    ///               type: string
    ///             password:
    ///               type: string
    ///   responses:
    ///     200:
    ///       description: Successfully authenticated
    ///     401:
    ///       description: Invalid credentials
    /// ```
    pub async fn login(
        State(state): State<Arc<ApiState>>,
        Json(request): Json<LoginRequest>,
    ) -> Result<Json<SuccessResponse<LoginResponse>>, ErrorResponse> {
        // Validate basic input
        if request.username.is_empty() || request.password.is_empty() {
            return Err(ErrorResponse::new(
                "Invalid credentials",
                "INVALID_CREDENTIALS",
            ));
        }

        // Get user from store
        let user = state
            .user_store
            .get_user_by_username(&request.username)
            .ok_or_else(|| ErrorResponse::new("Invalid credentials", "INVALID_CREDENTIALS"))?;

        // Verify password with Argon2
        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| ErrorResponse::new("Authentication failed", "INTERNAL_ERROR"))?;

        Argon2::default()
            .verify_password(request.password.as_bytes(), &parsed_hash)
            .map_err(|_| ErrorResponse::new("Invalid credentials", "INVALID_CREDENTIALS"))?;

        // Create JWT claims
        let permissions = vec!["simulator:execute".to_string(), "ruvector:search".to_string()];
        let claims = UserClaims::new(
            user.id.clone(),
            user.roles.clone(),
            permissions,
            "cognitum".to_string(),
            chrono::Duration::minutes(15),
        );

        // Create access token
        let access_token = state
            .jwt_service
            .create_access_token(&claims)
            .map_err(|e| ErrorResponse::new(format!("Token creation failed: {}", e), "INTERNAL_ERROR"))?;

        // Create refresh token
        let refresh_token = state
            .jwt_service
            .create_refresh_token(&user.id)
            .await
            .map_err(|e| ErrorResponse::new(format!("Token creation failed: {}", e), "INTERNAL_ERROR"))?;

        let response = LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: 900,
        };

        Ok(Json(SuccessResponse::new(response)))
    }

    /// Register endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// post:
    ///   summary: Register new user account
    ///   tags: [Authentication]
    ///   requestBody:
    ///     required: true
    ///     content:
    ///       application/json:
    ///         schema:
    ///           type: object
    ///           required: [username, password]
    ///           properties:
    ///             username:
    ///               type: string
    ///             password:
    ///               type: string
    ///             tier:
    ///               type: string
    ///               enum: [free, pro, enterprise]
    ///   responses:
    ///     201:
    ///       description: User registered successfully
    ///     400:
    ///       description: Invalid request or user already exists
    /// ```
    pub async fn register(
        State(state): State<Arc<ApiState>>,
        Json(request): Json<RegisterRequest>,
    ) -> Result<Json<SuccessResponse<RegisterResponse>>, ErrorResponse> {
        // Validate input
        if request.username.is_empty() || request.password.is_empty() {
            return Err(ErrorResponse::new(
                "Username and password are required",
                "INVALID_REQUEST",
            ));
        }

        if request.password.len() < 8 {
            return Err(ErrorResponse::new(
                "Password must be at least 8 characters",
                "INVALID_REQUEST",
            ));
        }

        // Hash password with Argon2
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(request.password.as_bytes(), &salt)
            .map_err(|e| ErrorResponse::new(format!("Password hashing failed: {}", e), "INTERNAL_ERROR"))?
            .to_string();

        // Create user with default tier "free"
        let tier = request.tier.unwrap_or_else(|| "free".to_string());
        let user = state
            .user_store
            .create_user(&request.username, &password_hash, &tier)
            .map_err(|e| ErrorResponse::new(e, "INVALID_REQUEST"))?;

        let response = RegisterResponse {
            user_id: user.id,
            username: user.username,
        };

        Ok(Json(SuccessResponse::new(response)))
    }

    /// Refresh token endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// post:
    ///   summary: Refresh access token using refresh token
    ///   tags: [Authentication]
    ///   requestBody:
    ///     required: true
    ///     content:
    ///       application/json:
    ///         schema:
    ///           type: object
    ///           required: [refresh_token]
    ///           properties:
    ///             refresh_token:
    ///               type: string
    ///   responses:
    ///     200:
    ///       description: Tokens refreshed successfully
    ///     401:
    ///       description: Invalid or expired refresh token
    /// ```
    pub async fn refresh_token(
        State(state): State<Arc<ApiState>>,
        Json(request): Json<RefreshTokenRequest>,
    ) -> Result<Json<SuccessResponse<LoginResponse>>, ErrorResponse> {
        // Refresh tokens using JWT service
        let (new_access_token, new_refresh_token) = state
            .jwt_service
            .refresh_tokens(&request.refresh_token)
            .await
            .map_err(|e| match e {
                crate::auth::AuthError::TokenExpired => {
                    ErrorResponse::new("Refresh token expired", "TOKEN_EXPIRED")
                }
                crate::auth::AuthError::TokenReplayDetected => {
                    ErrorResponse::new("Token replay detected", "INVALID_TOKEN")
                }
                _ => ErrorResponse::new("Invalid refresh token", "INVALID_TOKEN"),
            })?;

        let response = LoginResponse {
            access_token: new_access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: 900,
        };

        Ok(Json(SuccessResponse::new(response)))
    }

    /// Create API key endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// post:
    ///   summary: Create new API key
    ///   tags: [API Keys]
    ///   security:
    ///     - BearerAuth: []
    ///   requestBody:
    ///     required: true
    ///     content:
    ///       application/json:
    ///         schema:
    ///           type: object
    ///           required: [scope]
    ///           properties:
    ///             scope:
    ///               type: string
    ///               enum: [ReadOnly, ReadWrite, Admin]
    ///             description:
    ///               type: string
    ///   responses:
    ///     201:
    ///       description: API key created successfully
    ///     401:
    ///       description: Unauthorized
    /// ```
    pub async fn create_api_key(
        State(state): State<Arc<ApiState>>,
        Extension(user): Extension<crate::api::handlers::AuthenticatedUser>,
        Json(request): Json<CreateApiKeyRequest>,
    ) -> Result<Json<SuccessResponse<CreateApiKeyResponse>>, ErrorResponse> {
        // Extract user_id from authenticated user extension
        let user_id = UserId::new(&user.user_id);

        let (api_key, key_id) = state
            .api_key_service
            .create_key(&user_id, request.scope)
            .await
            .map_err(|e| ErrorResponse::new(format!("Failed to create API key: {}", e), "INTERNAL_ERROR"))?;

        let response = CreateApiKeyResponse {
            api_key,
            key_id,
            scope: request.scope,
            created_at: chrono::Utc::now(),
        };

        Ok(Json(SuccessResponse::new(response)))
    }

    /// Revoke API key endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// delete:
    ///   summary: Revoke API key
    ///   tags: [API Keys]
    ///   security:
    ///     - BearerAuth: []
    ///   parameters:
    ///     - name: id
    ///       in: path
    ///       required: true
    ///       schema:
    ///         type: string
    ///   responses:
    ///     204:
    ///       description: API key revoked successfully
    ///     401:
    ///       description: Unauthorized
    ///     404:
    ///       description: API key not found
    /// ```
    pub async fn revoke_api_key(
        State(state): State<Arc<ApiState>>,
        Path(key_id): Path<String>,
    ) -> Result<StatusCode, ErrorResponse> {
        state
            .api_key_service
            .revoke_key(&key_id, "Revoked by user")
            .await
            .map_err(|e| ErrorResponse::new(format!("Failed to revoke key: {}", e), "INTERNAL_ERROR"))?;

        Ok(StatusCode::NO_CONTENT)
    }
}

/// Simulator handlers module
pub mod simulator_handlers {
    use super::*;

    /// Execute simulation request
    #[derive(Debug, Deserialize)]
    pub struct ExecuteSimulationRequest {
        pub program: String,
        pub inputs: Vec<serde_json::Value>,
        #[serde(default)]
        pub max_steps: Option<u64>,
    }

    /// Execute simulation response
    #[derive(Debug, Serialize)]
    pub struct ExecuteSimulationResponse {
        pub simulation_id: String,
        pub status: String,
        pub result: Option<serde_json::Value>,
    }

    /// Simulator status response
    #[derive(Debug, Serialize)]
    pub struct SimulatorStatusResponse {
        pub status: String,
        pub version: String,
        pub uptime_seconds: u64,
    }

    /// Execute simulation endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// post:
    ///   summary: Execute a simulation
    ///   tags: [Simulator]
    ///   security:
    ///     - BearerAuth: []
    ///     - ApiKeyAuth: []
    ///   requestBody:
    ///     required: true
    ///     content:
    ///       application/json:
    ///         schema:
    ///           type: object
    ///           required: [program, inputs]
    ///           properties:
    ///             program:
    ///               type: string
    ///             inputs:
    ///               type: array
    ///             max_steps:
    ///               type: integer
    ///   responses:
    ///     200:
    ///       description: Simulation executed successfully
    ///     401:
    ///       description: Unauthorized
    /// ```
    pub async fn execute_simulation(
        State(state): State<Arc<ApiState>>,
        Extension(_user): Extension<crate::api::handlers::AuthenticatedUser>,
        Json(request): Json<ExecuteSimulationRequest>,
    ) -> Result<Json<SuccessResponse<ExecuteSimulationResponse>>, ErrorResponse> {
        // Validate request
        if request.program.is_empty() {
            return Err(ErrorResponse::new(
                "Program cannot be empty",
                "INVALID_REQUEST",
            ));
        }

        // Parse program as hex string to bytes
        let bytecode = hex::decode(&request.program)
            .map_err(|e| ErrorResponse::new(
                format!("Invalid program format (expected hex): {}", e),
                "INVALID_REQUEST",
            ))?;

        // Lock simulator and execute
        let mut simulator = state.simulator.lock().await;

        // Load program
        let _handle = simulator
            .create_program(&bytecode)
            .await
            .map_err(|e| ErrorResponse::new(
                format!("Failed to load program: {}", e),
                "INVALID_REQUEST",
            ))?;

        // Execute simulation
        let max_cycles = request.max_steps.unwrap_or(10000);
        let result = simulator
            .execute(Some(max_cycles))
            .await
            .map_err(|e| ErrorResponse::new(
                format!("Simulation execution failed: {}", e),
                "INTERNAL_ERROR",
            ))?;

        // Build response with actual results
        let result_json = serde_json::json!({
            "cycles_executed": result.cycles_executed,
            "instructions_executed": result.instructions_executed,
            "halted": result.halted,
            "status": if result.halted { "completed" } else { "timeout" },
        });

        let response = ExecuteSimulationResponse {
            simulation_id: uuid::Uuid::new_v4().to_string(),
            status: "completed".to_string(),
            result: Some(result_json),
        };

        Ok(Json(SuccessResponse::new(response)))
    }

    /// Get simulator status endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// get:
    ///   summary: Get simulator status
    ///   tags: [Simulator]
    ///   security:
    ///     - BearerAuth: []
    ///     - ApiKeyAuth: []
    ///   responses:
    ///     200:
    ///       description: Simulator status retrieved
    ///     401:
    ///       description: Unauthorized
    /// ```
    pub async fn get_status(
        State(state): State<Arc<ApiState>>,
        Extension(_user): Extension<crate::api::handlers::AuthenticatedUser>,
    ) -> Result<Json<SuccessResponse<SimulatorStatusResponse>>, ErrorResponse> {
        let simulator = state.simulator.lock().await;
        let snapshot = simulator.get_snapshot().await
            .map_err(|e| ErrorResponse::new(
                format!("Failed to get simulator status: {}", e),
                "INTERNAL_ERROR",
            ))?;

        let response = SimulatorStatusResponse {
            status: "running".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: snapshot.cycles,
        };

        Ok(Json(SuccessResponse::new(response)))
    }
}

/// Ruvector handlers module
pub mod ruvector_handlers {
    use super::*;

    /// Vector search request
    #[derive(Debug, Deserialize)]
    pub struct VectorSearchRequest {
        pub query_vector: Vec<f32>,
        pub top_k: usize,
        #[serde(default)]
        pub filter: Option<serde_json::Value>,
    }

    /// Vector search response
    #[derive(Debug, Serialize)]
    pub struct VectorSearchResponse {
        pub results: Vec<SearchResult>,
        pub took_ms: u64,
    }

    /// Single search result
    #[derive(Debug, Serialize)]
    pub struct SearchResult {
        pub id: String,
        pub score: f32,
        pub metadata: Option<serde_json::Value>,
    }

    /// Insert vectors request
    #[derive(Debug, Deserialize)]
    pub struct InsertVectorsRequest {
        pub vectors: Vec<VectorRecord>,
    }

    /// Vector record
    #[derive(Debug, Deserialize)]
    pub struct VectorRecord {
        pub id: String,
        pub vector: Vec<f32>,
        #[serde(default)]
        pub metadata: Option<serde_json::Value>,
    }

    /// Insert vectors response
    #[derive(Debug, Serialize)]
    pub struct InsertVectorsResponse {
        pub inserted_count: usize,
        pub failed_ids: Vec<String>,
    }

    /// Vector search endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// post:
    ///   summary: Perform vector similarity search
    ///   tags: [Ruvector]
    ///   security:
    ///     - BearerAuth: []
    ///     - ApiKeyAuth: []
    ///   requestBody:
    ///     required: true
    ///     content:
    ///       application/json:
    ///         schema:
    ///           type: object
    ///           required: [query_vector, top_k]
    ///           properties:
    ///             query_vector:
    ///               type: array
    ///               items:
    ///                 type: number
    ///             top_k:
    ///               type: integer
    ///             filter:
    ///               type: object
    ///   responses:
    ///     200:
    ///       description: Search completed successfully
    ///     401:
    ///       description: Unauthorized
    /// ```
    pub async fn vector_search(
        State(state): State<Arc<ApiState>>,
        Extension(_user): Extension<crate::api::handlers::AuthenticatedUser>,
        Json(request): Json<VectorSearchRequest>,
    ) -> Result<Json<SuccessResponse<VectorSearchResponse>>, ErrorResponse> {
        // Validate request
        if request.query_vector.is_empty() {
            return Err(ErrorResponse::new(
                "Query vector cannot be empty",
                "INVALID_REQUEST",
            ));
        }

        if request.top_k == 0 {
            return Err(ErrorResponse::new("top_k must be greater than 0", "INVALID_REQUEST"));
        }

        // Create embedding from query vector
        let query_embedding = crate::ruvector::types::Embedding::new(request.query_vector);

        // Perform search with timing
        let start = std::time::Instant::now();
        let search_results = state
            .ruvector
            .search_similar(&query_embedding, request.top_k)
            .map_err(|e| ErrorResponse::new(
                format!("Vector search failed: {}", e),
                "INTERNAL_ERROR",
            ))?;
        let took_ms = start.elapsed().as_millis() as u64;

        // Convert to response format
        let results = search_results
            .into_iter()
            .map(|r| SearchResult {
                id: r.id.0.to_string(),
                score: r.similarity,
                metadata: Some(serde_json::to_value(&r.metadata).unwrap_or(serde_json::Value::Null)),
            })
            .collect();

        let response = VectorSearchResponse {
            results,
            took_ms,
        };

        Ok(Json(SuccessResponse::new(response)))
    }

    /// Insert vectors endpoint
    ///
    /// # OpenAPI
    /// ```yaml
    /// post:
    ///   summary: Insert vectors into index
    ///   tags: [Ruvector]
    ///   security:
    ///     - BearerAuth: []
    ///     - ApiKeyAuth: []
    ///   requestBody:
    ///     required: true
    ///     content:
    ///       application/json:
    ///         schema:
    ///           type: object
    ///           required: [vectors]
    ///           properties:
    ///             vectors:
    ///               type: array
    ///               items:
    ///                 type: object
    ///                 required: [id, vector]
    ///                 properties:
    ///                   id:
    ///                     type: string
    ///                   vector:
    ///                     type: array
    ///                     items:
    ///                       type: number
    ///                   metadata:
    ///                     type: object
    ///   responses:
    ///     200:
    ///       description: Vectors inserted successfully
    ///     401:
    ///       description: Unauthorized
    /// ```
    pub async fn insert_vectors(
        State(state): State<Arc<ApiState>>,
        Extension(_user): Extension<crate::api::handlers::AuthenticatedUser>,
        Json(request): Json<InsertVectorsRequest>,
    ) -> Result<Json<SuccessResponse<InsertVectorsResponse>>, ErrorResponse> {
        // Validate request
        if request.vectors.is_empty() {
            return Err(ErrorResponse::new(
                "Vectors array cannot be empty",
                "INVALID_REQUEST",
            ));
        }

        let mut inserted_count = 0;
        let mut failed_ids = Vec::new();

        // Insert each vector
        for record in request.vectors {
            // Validate vector dimensions
            if record.vector.is_empty() {
                failed_ids.push(record.id.clone());
                continue;
            }

            // Parse ID as u64
            let embedding_id = match record.id.parse::<u64>() {
                Ok(id) => crate::ruvector::types::EmbeddingId(id),
                Err(_) => {
                    failed_ids.push(record.id);
                    continue;
                }
            };

            // Create embedding
            let embedding = crate::ruvector::types::Embedding::new(record.vector);

            // Create metadata
            let mut metadata = crate::ruvector::types::Metadata::default();
            if let Some(meta_value) = record.metadata {
                if let Some(obj) = meta_value.as_object() {
                    for (key, value) in obj {
                        if let Some(s) = value.as_str() {
                            metadata.custom.insert(key.clone(), s.to_string());
                        }
                    }
                }
            }

            // Insert into index
            match state.ruvector.store_embedding(embedding_id, &embedding, &metadata) {
                Ok(_) => inserted_count += 1,
                Err(_) => failed_ids.push(record.id),
            }
        }

        let response = InsertVectorsResponse {
            inserted_count,
            failed_ids,
        };

        Ok(Json(SuccessResponse::new(response)))
    }
}
