//! Authentication and Authorization module for Cognitum chip v1
//!
//! This module provides:
//! - API key authentication with Argon2 hashing
//! - JWT token authentication with Ed25519 signing
//! - Role-Based Access Control (RBAC)
//! - Permission management
//! - Tier-based access control
//! - Resource-level authorization
//! - Refresh token rotation for enhanced security
//! - Constant-time validation to prevent timing attacks
//!
//! # Security Features
//!
//! - API keys are hashed with Argon2 before storage (never stored in plaintext)
//! - JWT tokens are signed with Ed25519 for cryptographic verification
//! - Refresh tokens are automatically rotated on each use
//! - Token replay detection triggers family revocation
//! - Constant-time comparisons prevent timing attacks

pub mod api_keys;
pub mod errors;
pub mod jwt;
pub mod rbac;
pub mod roles;
pub mod types;

// Re-export commonly used types
pub use api_keys::{ApiKeyService, ApiKeyStore};
pub use errors::{AuthError, AuthResult, StoreError as AuthStoreError, StoreResult};
pub use jwt::{JwtConfig, JwtService, TokenStore};
pub use rbac::{
    Permission, PermissionStore, RbacError, RbacService, ResourceId, StoreError, User, UserId,
};
pub use roles::{RoleManager, Tier};
pub use types::{KeyMetadata, KeyScope, TokenMetadata, UserClaims};
