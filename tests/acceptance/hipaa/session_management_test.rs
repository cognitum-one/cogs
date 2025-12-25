//! Acceptance tests for HIPAA session management
//!
//! Validates HIPAA §164.312(a)(2)(iii) - Automatic logoff

use cognitum::hipaa::*;
use std::time::Duration;

#[tokio::test]
async fn should_auto_logout_after_15_minutes_inactivity() {
    // Given: HIPAA session manager with 15-minute timeout
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_secs(900), // 15 minutes
        absolute_timeout: Duration::from_secs(28800), // 8 hours
    });

    let user_id = UserId::new("clinician_001");
    let session = manager.create_session(&user_id).await.unwrap();

    // When: Session is inactive for 15 minutes (simulated with short timeout)
    let short_timeout_manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_millis(100),
        absolute_timeout: Duration::from_secs(3600),
    });

    let test_session = short_timeout_manager.create_session(&user_id).await.unwrap();
    assert!(short_timeout_manager.is_valid(&test_session.id).await.unwrap());

    tokio::time::sleep(Duration::from_millis(150)).await;

    // Then: Session should be automatically invalidated
    assert!(
        !short_timeout_manager.is_valid(&test_session.id).await.unwrap(),
        "Session should be invalid after inactivity timeout"
    );
}

#[tokio::test]
async fn should_enforce_absolute_timeout_of_8_hours() {
    // Given: HIPAA session manager with 8-hour absolute timeout
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_secs(900),
        absolute_timeout: Duration::from_secs(28800), // 8 hours
    });

    // When: Session is kept active but exceeds absolute timeout (simulated)
    let short_absolute_manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_secs(3600),
        absolute_timeout: Duration::from_millis(200),
    });

    let user_id = UserId::new("user_001");
    let session = short_absolute_manager.create_session(&user_id).await.unwrap();

    // Keep updating activity
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        short_absolute_manager.update_activity(&session.id).await.unwrap();
    }

    // Wait for absolute timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Then: Session should be invalid despite activity
    assert!(
        !short_absolute_manager.is_valid(&session.id).await.unwrap(),
        "Session should expire after absolute timeout regardless of activity"
    );
}

#[tokio::test]
async fn should_generate_unique_session_identifiers() {
    // Given: HIPAA session manager
    let manager = HipaaSessionManager::new(SessionConfig::default());
    let user_id = UserId::new("user_001");

    // When: Creating multiple sessions for same user
    let session1 = manager.create_session(&user_id).await.unwrap();
    let session2 = manager.create_session(&user_id).await.unwrap();
    let session3 = manager.create_session(&user_id).await.unwrap();

    // Then: Each session should have unique identifier
    assert_ne!(session1.id, session2.id);
    assert_ne!(session2.id, session3.id);
    assert_ne!(session1.id, session3.id);
}

#[tokio::test]
async fn should_allow_manual_session_termination() {
    // Given: Active user session
    let manager = HipaaSessionManager::new(SessionConfig::default());
    let user_id = UserId::new("user_001");
    let session = manager.create_session(&user_id).await.unwrap();

    assert!(manager.is_valid(&session.id).await.unwrap());

    // When: Session is manually terminated (user logout)
    manager.invalidate_session(&session.id).await.unwrap();

    // Then: Session should be immediately invalid
    assert!(
        !manager.is_valid(&session.id).await.unwrap(),
        "Manually terminated session should be invalid"
    );
}

#[tokio::test]
async fn should_extend_session_on_activity() {
    // Given: Session approaching inactivity timeout
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_millis(200),
        absolute_timeout: Duration::from_secs(3600),
    });

    let user_id = UserId::new("user_001");
    let session = manager.create_session(&user_id).await.unwrap();

    // When: User is active just before timeout
    tokio::time::sleep(Duration::from_millis(100)).await;
    manager.update_activity(&session.id).await.unwrap();

    // Wait what would have been a timeout
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Then: Session should still be valid (activity reset the timer)
    assert!(
        manager.is_valid(&session.id).await.unwrap(),
        "Activity should extend inactivity timeout"
    );
}

#[tokio::test]
async fn should_cleanup_expired_sessions() {
    // Given: Multiple expired sessions
    let manager = HipaaSessionManager::new(SessionConfig {
        inactivity_timeout: Duration::from_millis(50),
        absolute_timeout: Duration::from_secs(3600),
    });

    // Create 5 sessions
    for i in 0..5 {
        let user_id = UserId::new(format!("user_{}", i));
        manager.create_session(&user_id).await.unwrap();
    }

    assert_eq!(manager.active_session_count(), 5);

    // When: Sessions expire and cleanup runs
    tokio::time::sleep(Duration::from_millis(100)).await;
    let cleaned_count = manager.cleanup_expired().await;

    // Then: All expired sessions should be removed
    assert_eq!(cleaned_count, 5, "Should cleanup all expired sessions");
    assert_eq!(manager.active_session_count(), 0);
}

#[tokio::test]
async fn should_track_session_metadata() {
    // Given: HIPAA session manager
    let manager = HipaaSessionManager::new(SessionConfig::default());
    let user_id = UserId::new("user_001");

    // When: Creating a session
    let session = manager.create_session(&user_id).await.unwrap();

    // Then: Session should contain required metadata
    assert_eq!(session.user_id, user_id);
    assert!(session.created_at <= chrono::Utc::now());
    assert!(session.expires_at > chrono::Utc::now());
    assert_eq!(session.last_activity, session.created_at);

    // When: Activity is updated
    tokio::time::sleep(Duration::from_millis(10)).await;
    manager.update_activity(&session.id).await.unwrap();

    // Then: Last activity should be updated
    let updated_session = manager.get_session(&session.id).await.unwrap();
    assert!(updated_session.last_activity > session.created_at);
}
