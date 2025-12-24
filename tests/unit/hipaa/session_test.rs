//! Unit tests for HIPAA session management

use cognitum::hipaa::*;
use std::time::Duration;

#[tokio::test]
async fn session_creation_and_validation() {
    let manager = HipaaSessionManager::new(SessionConfig::default());
    let user_id = UserId::new("user_123");

    let session = manager.create_session(&user_id).await.unwrap();

    assert_eq!(session.user_id, user_id);
    assert!(manager.is_valid(&session.id).await.unwrap());
}

#[tokio::test]
async fn inactivity_timeout_15_minutes() {
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_millis(100), // Short timeout for testing
        absolute_timeout: Duration::from_secs(3600),
    });

    let user_id = UserId::new("user_123");
    let session = manager.create_session(&user_id).await.unwrap();

    // Session is valid initially
    assert!(manager.is_valid(&session.id).await.unwrap());

    // Wait for inactivity timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Session should be invalid due to inactivity
    assert!(!manager.is_valid(&session.id).await.unwrap());
}

#[tokio::test]
async fn absolute_timeout_8_hours() {
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_secs(3600),
        absolute_timeout: Duration::from_millis(200), // Short for testing
    });

    let user_id = UserId::new("user_123");
    let session = manager.create_session(&user_id).await.unwrap();

    // Keep updating activity
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = manager.update_activity(&session.id).await;
    }

    // Wait for absolute timeout
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Session should be invalid due to absolute timeout
    assert!(!manager.is_valid(&session.id).await.unwrap());
}

#[tokio::test]
async fn activity_update_extends_inactivity() {
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_millis(200),
        absolute_timeout: Duration::from_secs(3600),
    });

    let user_id = UserId::new("user_123");
    let session = manager.create_session(&user_id).await.unwrap();

    // Wait almost to timeout
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Update activity
    manager.update_activity(&session.id).await.unwrap();

    // Wait another 100ms (would timeout without activity update)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Session should still be valid
    assert!(manager.is_valid(&session.id).await.unwrap());
}

#[tokio::test]
async fn manual_session_invalidation() {
    let manager = HipaaSessionManager::new(SessionConfig::default());
    let user_id = UserId::new("user_123");

    let session = manager.create_session(&user_id).await.unwrap();
    assert!(manager.is_valid(&session.id).await.unwrap());

    // Manually invalidate
    manager.invalidate_session(&session.id).await.unwrap();

    // Session should be invalid
    assert!(!manager.is_valid(&session.id).await.unwrap());
}

#[tokio::test]
async fn cleanup_expired_sessions() {
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_millis(50),
        absolute_timeout: Duration::from_secs(3600),
    });

    // Create multiple sessions
    for i in 0..5 {
        let user_id = UserId::new(format!("user_{}", i));
        manager.create_session(&user_id).await.unwrap();
    }

    assert_eq!(manager.active_session_count(), 5);

    // Wait for inactivity timeout
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cleanup expired sessions
    let cleaned = manager.cleanup_expired().await;
    assert_eq!(cleaned, 5);
    assert_eq!(manager.active_session_count(), 0);
}

#[tokio::test]
async fn unique_session_identifiers() {
    let manager = HipaaSessionManager::new(SessionConfig::default());
    let user_id = UserId::new("user_123");

    let session1 = manager.create_session(&user_id).await.unwrap();
    let session2 = manager.create_session(&user_id).await.unwrap();

    assert_ne!(session1.id, session2.id);
}

#[tokio::test]
async fn get_session_info() {
    let manager = HipaaSessionManager::new(SessionConfig::default());
    let user_id = UserId::new("user_123");

    let created_session = manager.create_session(&user_id).await.unwrap();
    let retrieved_session = manager.get_session(&created_session.id).await.unwrap();

    assert_eq!(created_session.id, retrieved_session.id);
    assert_eq!(created_session.user_id, retrieved_session.user_id);
}
