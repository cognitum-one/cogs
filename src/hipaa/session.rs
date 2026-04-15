//! HIPAA-compliant session management with automatic timeouts
//!
//! Implements:
//! - 15-minute inactivity timeout
//! - 8-hour absolute timeout
//! - Automatic session invalidation

use super::{HipaaError, Result, SessionId, UserId};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Inactivity timeout (default: 15 minutes)
    pub inactivity_timeout: std::time::Duration,
    /// Absolute timeout (default: 8 hours)
    pub absolute_timeout: std::time::Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            inactivity_timeout: std::time::Duration::from_secs(900), // 15 minutes
            absolute_timeout: std::time::Duration::from_secs(28800),  // 8 hours
        }
    }
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub user_id: UserId,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Session {
    /// Check if session is expired
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now > self.expires_at
    }

    /// Check if session is inactive
    pub fn is_inactive(&self, now: DateTime<Utc>, timeout: std::time::Duration) -> bool {
        let timeout_chrono = Duration::from_std(timeout).unwrap();
        let inactive_threshold = self.last_activity + timeout_chrono;
        now > inactive_threshold
    }
}

/// HIPAA-compliant session manager
pub struct HipaaSessionManager {
    config: SessionConfig,
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
}

impl HipaaSessionManager {
    /// Create new session manager
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create new session for user
    pub async fn create_session(&self, user_id: &UserId) -> Result<Session> {
        let now = Utc::now();
        let absolute_timeout_chrono = Duration::from_std(self.config.absolute_timeout)
            .map_err(|_e| HipaaError::InvalidSession)?;

        let session = Session {
            id: SessionId::new(format!("sess_{}", uuid::Uuid::new_v4())),
            user_id: user_id.clone(),
            created_at: now,
            last_activity: now,
            expires_at: now + absolute_timeout_chrono,
        };

        self.sessions
            .write()
            .unwrap()
            .insert(session.id.clone(), session.clone());

        Ok(session)
    }

    /// Validate session
    pub async fn is_valid(&self, session_id: &SessionId) -> Result<bool> {
        let sessions = self.sessions.read().unwrap();
        let session = sessions.get(session_id);

        match session {
            None => Ok(false),
            Some(session) => {
                let now = Utc::now();

                // Check absolute timeout
                if session.is_expired(now) {
                    drop(sessions);
                    self.invalidate_session(session_id).await?;
                    return Ok(false);
                }

                // Check inactivity timeout
                if session.is_inactive(now, self.config.inactivity_timeout) {
                    drop(sessions);
                    self.invalidate_session(session_id).await?;
                    return Ok(false);
                }

                Ok(true)
            }
        }
    }

    /// Update session activity
    pub async fn update_activity(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_activity = Utc::now();
            Ok(())
        } else {
            Err(HipaaError::InvalidSession)
        }
    }

    /// Invalidate session
    pub async fn invalidate_session(&self, session_id: &SessionId) -> Result<()> {
        self.sessions.write().unwrap().remove(session_id);
        Ok(())
    }

    /// Get session
    pub async fn get_session(&self, session_id: &SessionId) -> Result<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(session_id)
            .cloned()
            .ok_or(HipaaError::InvalidSession)
    }

    /// Get active session count
    pub fn active_session_count(&self) -> usize {
        self.sessions.read().unwrap().len()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let mut sessions = self.sessions.write().unwrap();

        let expired: Vec<SessionId> = sessions
            .iter()
            .filter(|(_, session)| {
                session.is_expired(now) || session.is_inactive(now, self.config.inactivity_timeout)
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for id in expired {
            sessions.remove(&id);
        }

        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        let manager = HipaaSessionManager::new(SessionConfig::default());
        let user_id = UserId::new("user_123");

        let session = manager.create_session(&user_id).await.unwrap();

        assert_eq!(session.user_id, user_id);
        assert!(manager.is_valid(&session.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_inactivity_timeout() {
        let manager = HipaaSessionManager::new(SessionConfig {
            inactivity_timeout: std::time::Duration::from_secs(1),
            absolute_timeout: std::time::Duration::from_secs(3600),
        });

        let user_id = UserId::new("user_123");
        let session = manager.create_session(&user_id).await.unwrap();

        // Session is valid initially
        assert!(manager.is_valid(&session.id).await.unwrap());

        // Wait for inactivity timeout
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Session should be invalid due to inactivity
        assert!(!manager.is_valid(&session.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_absolute_timeout() {
        let manager = HipaaSessionManager::new(SessionConfig {
            inactivity_timeout: std::time::Duration::from_secs(3600),
            absolute_timeout: std::time::Duration::from_secs(1),
        });

        let user_id = UserId::new("user_123");
        let session = manager.create_session(&user_id).await.unwrap();

        // Keep updating activity
        for _ in 0..3 {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            let _ = manager.update_activity(&session.id).await;
        }

        // Wait for absolute timeout
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Session should be invalid due to absolute timeout
        assert!(!manager.is_valid(&session.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_session_invalidation() {
        let manager = HipaaSessionManager::new(SessionConfig::default());
        let user_id = UserId::new("user_123");

        let session = manager.create_session(&user_id).await.unwrap();
        assert!(manager.is_valid(&session.id).await.unwrap());

        manager.invalidate_session(&session.id).await.unwrap();
        assert!(!manager.is_valid(&session.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let manager = HipaaSessionManager::new(SessionConfig {
            inactivity_timeout: std::time::Duration::from_millis(100),
            absolute_timeout: std::time::Duration::from_secs(3600),
        });

        // Create multiple sessions
        for i in 0..5 {
            let user_id = UserId::new(format!("user_{}", i));
            manager.create_session(&user_id).await.unwrap();
        }

        assert_eq!(manager.active_session_count(), 5);

        // Wait for inactivity timeout
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Cleanup expired sessions
        let cleaned = manager.cleanup_expired().await;
        assert_eq!(cleaned, 5);
        assert_eq!(manager.active_session_count(), 0);
    }
}
