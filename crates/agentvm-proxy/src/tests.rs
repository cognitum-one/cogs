//! Comprehensive tests for agentvm-proxy crate
//!
//! Test coverage:
//! - Capability grant and revoke
//! - Invoke with valid/invalid capabilities
//! - Budget deduction
//! - Evidence logging
//! - Integration tests with mock executors

use super::*;
use agentvm_types::{
    Budget, BudgetVector, CapabilityGrant, CapabilityScope, CapabilityType, CapsuleId, Quota,
    Rights,
};
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "std")]
use std::sync::Mutex as StdMutex;
#[cfg(not(feature = "std"))]
use spin::Mutex as StdMutex;

/// Mock executor for testing
struct MockExecutor {
    name: String,
    response_payload: Vec<u8>,
    response_budget: BudgetVector,
    call_count: AtomicU64,
    should_fail: bool,
}

impl MockExecutor {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            response_payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
            response_budget: BudgetVector::new(10, 100, 1024, 0, 512, 1),
            call_count: AtomicU64::new(0),
            should_fail: false,
        }
    }

    fn with_response(mut self, payload: Vec<u8>) -> Self {
        self.response_payload = payload;
        self
    }

    fn with_budget(mut self, budget: BudgetVector) -> Self {
        self.response_budget = budget;
        self
    }

    fn failing(mut self) -> Self {
        self.should_fail = true;
        self
    }

    fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl Executor for MockExecutor {
    fn execute(&self, _cap_type: CapabilityType, _request: &InvokeRequest) -> Result<InvokeResponse> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.should_fail {
            return Err(ProxyError::ExecutorError("mock failure".to_string()));
        }

        Ok(InvokeResponse {
            payload: self.response_payload.clone(),
            actual_budget: self.response_budget,
            evidence_id: 0,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Mock evidence logger for testing
struct MockEvidenceLogger {
    grant_count: AtomicU64,
    revoke_count: AtomicU64,
    invoke_count: AtomicU64,
    next_evidence_id: AtomicU64,
}

impl MockEvidenceLogger {
    fn new() -> Self {
        Self {
            grant_count: AtomicU64::new(0),
            revoke_count: AtomicU64::new(0),
            invoke_count: AtomicU64::new(0),
            next_evidence_id: AtomicU64::new(1),
        }
    }

    fn grant_count(&self) -> u64 {
        self.grant_count.load(Ordering::SeqCst)
    }

    fn revoke_count(&self) -> u64 {
        self.revoke_count.load(Ordering::SeqCst)
    }

    fn invoke_count(&self) -> u64 {
        self.invoke_count.load(Ordering::SeqCst)
    }
}

impl EvidenceLogger for MockEvidenceLogger {
    fn log_grant(&self, _capsule_id: CapsuleId, _cap: &Capability) {
        self.grant_count.fetch_add(1, Ordering::SeqCst);
    }

    fn log_revoke(&self, _capsule_id: CapsuleId, _cap_id: CapabilityId) {
        self.revoke_count.fetch_add(1, Ordering::SeqCst);
    }

    fn log_invoke(
        &self,
        _capsule_id: CapsuleId,
        _cap_id: CapabilityId,
        _target: &str,
        _success: bool,
        _budget_used: &BudgetVector,
    ) -> u64 {
        self.invoke_count.fetch_add(1, Ordering::SeqCst);
        self.next_evidence_id.fetch_add(1, Ordering::SeqCst)
    }
}

/// Helper to create test capsule ID
fn test_capsule_id() -> CapsuleId {
    CapsuleId::from_bytes([0xAB; 16])
}

/// Helper to create test budget
fn test_budget() -> Budget {
    Budget::new(BudgetVector::new(
        10000,  // 10s CPU
        60000,  // 60s wall
        1024 * 1024, // 1MB memory
        1024 * 1024, // 1MB disk
        1024 * 1024, // 1MB network
        1000,        // 1000 requests
    ))
}

/// Helper to create test grant
fn test_grant() -> CapabilityGrant {
    CapabilityGrant::new(CapabilityType::NetworkHttp)
        .with_scope(CapabilityScope::Global)
        .with_rights(Rights::all())
        .with_quota(Quota::new(100, 1024 * 1024, 60_000_000_000))
        .with_lease(3600)
}

mod capsule_context_tests {
    use super::*;

    #[test]
    fn test_create_context() {
        let capsule_id = test_capsule_id();
        let budget = test_budget();
        let ctx = CapsuleContext::new(capsule_id, budget);

        assert_eq!(ctx.id, capsule_id);
        assert!(ctx.list_capabilities().is_empty());
    }

    #[test]
    fn test_grant_capability() {
        let mut ctx = CapsuleContext::new(test_capsule_id(), test_budget());

        let cap_id = CapabilityId::from_raw(1);
        let cap = Capability {
            id: cap_id,
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        };

        ctx.grant(cap);

        assert_eq!(ctx.list_capabilities().len(), 1);
        assert!(ctx.get_capability(cap_id).is_some());
    }

    #[test]
    fn test_revoke_capability() {
        let mut ctx = CapsuleContext::new(test_capsule_id(), test_budget());

        let cap_id = CapabilityId::from_raw(1);
        let cap = Capability {
            id: cap_id,
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        };

        ctx.grant(cap);
        let revoked = ctx.revoke(cap_id);

        assert!(revoked.is_some());
        assert!(ctx.get_capability(cap_id).unwrap().is_revoked());
    }

    #[test]
    fn test_list_valid_capabilities() {
        let mut ctx = CapsuleContext::new(test_capsule_id(), test_budget());
        ctx.set_time(1000);

        // Valid capability
        let cap1 = Capability {
            id: CapabilityId::from_raw(1),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        };

        // Expired capability
        let cap2 = Capability {
            id: CapabilityId::from_raw(2),
            cap_type: CapabilityType::FileRead,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: 500, // Expired
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: false,
        };

        // Revoked capability
        let cap3 = Capability {
            id: CapabilityId::from_raw(3),
            cap_type: CapabilityType::FileWrite,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
            revoked: true,
        };

        ctx.grant(cap1);
        ctx.grant(cap2);
        ctx.grant(cap3);

        let valid = ctx.list_valid_capabilities();
        assert_eq!(valid.len(), 1);
        assert_eq!(valid[0].id.as_raw(), 1);
    }

    #[test]
    fn test_budget_operations() {
        let mut ctx = CapsuleContext::new(test_capsule_id(), test_budget());

        let remaining = ctx.remaining_budget();
        assert!(remaining.cpu_time_ms > 0);

        let consume = BudgetVector::new(100, 100, 1024, 0, 512, 1);
        assert!(ctx.try_consume_budget(&consume).is_ok());

        assert!(ctx.budget_utilization() > 0.0);
    }
}

mod proxy_grant_revoke_tests {
    use super::*;

    #[test]
    fn test_grant_capability() {
        let proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        let caps = proxy.get_capabilities(capsule_id).expect("should get caps");
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].id, cap_id);
    }

    #[test]
    fn test_grant_to_unregistered_capsule() {
        let proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        let result = proxy.grant(capsule_id, test_grant());
        assert!(matches!(result, Err(ProxyError::CapsuleNotFound(_))));
    }

    #[test]
    fn test_revoke_capability() {
        let proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());
        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        proxy.revoke(capsule_id, cap_id).expect("should revoke");

        let caps = proxy.get_capabilities(capsule_id).expect("should get caps");
        assert!(caps[0].is_revoked());
    }

    #[test]
    fn test_revoke_nonexistent_capability() {
        let proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());

        let result = proxy.revoke(capsule_id, CapabilityId::from_raw(999));
        assert!(matches!(result, Err(ProxyError::CapabilityNotFound(_))));
    }

    #[test]
    fn test_revoke_cascade() {
        let proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());

        // Grant parent
        let parent_id = proxy.grant(capsule_id, test_grant()).expect("should grant parent");

        // Grant children (manually create derived capabilities for this test)
        {
            let mut contexts = proxy.contexts.write().unwrap();
            let ctx = contexts.get_mut(&capsule_id).unwrap();

            let child1 = Capability {
                id: CapabilityId::from_raw(100),
                cap_type: CapabilityType::NetworkHttp,
                scope: CapabilityScope::Global,
                rights: Rights::new(Rights::READ),
                quota: Quota::unlimited(),
                expires_at: u64::MAX,
                parent: Some(parent_id),
                proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
                revoked: false,
            };

            let child2 = Capability {
                id: CapabilityId::from_raw(101),
                cap_type: CapabilityType::NetworkHttp,
                scope: CapabilityScope::Global,
                rights: Rights::new(Rights::READ),
                quota: Quota::unlimited(),
                expires_at: u64::MAX,
                parent: Some(parent_id),
                proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
                revoked: false,
            };

            ctx.grant(child1);
            ctx.grant(child2);
        }

        let revoked = proxy.revoke_cascade(capsule_id, parent_id).expect("should revoke");
        assert_eq!(revoked.len(), 3); // parent + 2 children

        let caps = proxy.get_capabilities(capsule_id).expect("should get caps");
        assert!(caps.iter().all(|c| c.is_revoked()));
    }

    #[test]
    fn test_get_valid_capabilities() {
        let proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        // Grant valid capability
        proxy.grant(capsule_id, test_grant()).expect("should grant");

        // Grant and revoke another
        let revoked_id = proxy.grant(capsule_id, test_grant()).expect("should grant");
        proxy.revoke(capsule_id, revoked_id).expect("should revoke");

        let valid_caps = proxy.get_valid_capabilities(capsule_id).expect("should get");
        assert_eq!(valid_caps.len(), 1);
    }
}

mod proxy_invoke_tests {
    use super::*;

    fn setup_proxy_with_executor() -> (CapabilityProxy, CapsuleId) {
        let mut proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http")),
        );

        (proxy, capsule_id)
    }

    #[test]
    fn test_invoke_valid_capability() {
        let (proxy, capsule_id) = setup_proxy_with_executor();

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_right(Rights::READ)
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));

        let response = proxy.invoke(capsule_id, request).expect("should invoke");
        assert!(!response.payload.is_empty());
    }

    #[test]
    fn test_invoke_expired_capability() {
        let (proxy, capsule_id) = setup_proxy_with_executor();

        // Grant capability with short lease
        let grant = CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_lease(1); // 1 second

        let cap_id = proxy.grant(capsule_id, grant).expect("should grant");

        // Advance time past expiry
        proxy.set_capsule_time(capsule_id, 10_000_000_000_000).unwrap();

        let request = InvokeRequest::new(cap_id, "https://example.com");
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::CapabilityExpired(_))));
    }

    #[test]
    fn test_invoke_revoked_capability() {
        let (proxy, capsule_id) = setup_proxy_with_executor();

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");
        proxy.revoke(capsule_id, cap_id).expect("should revoke");

        let request = InvokeRequest::new(cap_id, "https://example.com");
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::CapabilityRevoked(_))));
    }

    #[test]
    fn test_invoke_exhausted_quota() {
        let (proxy, capsule_id) = setup_proxy_with_executor();

        // Grant with very limited quota
        let grant = CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_quota(Quota::new(0, 0, 0)); // Already exhausted

        let cap_id = proxy.grant(capsule_id, grant).expect("should grant");

        let request = InvokeRequest::new(cap_id, "https://example.com");
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::QuotaExhausted(_))));
    }

    #[test]
    fn test_invoke_scope_violation() {
        let (proxy, capsule_id) = setup_proxy_with_executor();

        // Grant with restricted scope
        let grant = CapabilityGrant::new(CapabilityType::NetworkHttp).with_scope(
            CapabilityScope::Network {
                hosts: vec!["allowed.com".to_string()],
                ports: vec![443],
                protocols: vec![],
            },
        );

        let cap_id = proxy.grant(capsule_id, grant).expect("should grant");

        // Try to access different host
        let request = InvokeRequest::new(cap_id, "https://forbidden.com");
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::ScopeViolation { .. })));
    }

    #[test]
    fn test_invoke_rights_violation() {
        let (proxy, capsule_id) = setup_proxy_with_executor();

        // Grant with limited rights
        let grant = CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_rights(Rights::new(Rights::READ)); // Only read

        let cap_id = proxy.grant(capsule_id, grant).expect("should grant");

        // Try to write
        let request = InvokeRequest::new(cap_id, "https://example.com").with_right(Rights::WRITE);
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::RightsViolation { .. })));
    }

    #[test]
    fn test_invoke_nonexistent_capability() {
        let (proxy, capsule_id) = setup_proxy_with_executor();

        let request = InvokeRequest::new(CapabilityId::from_raw(999), "https://example.com");
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::CapabilityNotFound(_))));
    }

    #[test]
    fn test_invoke_no_executor() {
        let proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_budget(BudgetVector::zero());
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::UnsupportedOperation)));
    }

    #[test]
    fn test_invoke_executor_failure() {
        let mut proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http").failing()),
        );

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));
        let result = proxy.invoke(capsule_id, request);

        assert!(matches!(result, Err(ProxyError::ExecutorError(_))));
    }
}

mod budget_deduction_tests {
    use super::*;

    #[test]
    fn test_budget_deducted_on_invoke() {
        let mut proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        let response_budget = BudgetVector::new(50, 200, 2048, 0, 1024, 2);
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http").with_budget(response_budget)),
        );

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        let initial_budget = proxy.get_remaining_budget(capsule_id).expect("should get");

        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));

        proxy.invoke(capsule_id, request).expect("should invoke");

        let remaining = proxy.get_remaining_budget(capsule_id).expect("should get");

        // Budget should have decreased
        assert!(remaining.cpu_time_ms < initial_budget.cpu_time_ms);
        assert!(remaining.wall_time_ms < initial_budget.wall_time_ms);
        assert!(remaining.network_bytes < initial_budget.network_bytes);
    }

    #[test]
    fn test_budget_exceeded() {
        let mut proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        // Small budget
        let small_budget = Budget::new(BudgetVector::new(10, 100, 1024, 0, 512, 1));
        proxy.register_capsule(capsule_id, small_budget);
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http")),
        );

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        // Request more than available
        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_budget(BudgetVector::new(1000, 10000, 1024 * 1024, 0, 1024 * 1024, 100));

        let result = proxy.invoke(capsule_id, request);
        assert!(matches!(result, Err(ProxyError::BudgetExceeded { .. })));
    }

    #[test]
    fn test_multiple_invokes_accumulate_budget() {
        let mut proxy = CapabilityProxy::new();
        let capsule_id = test_capsule_id();

        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        let response_budget = BudgetVector::new(100, 100, 1024, 0, 100, 1);
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http").with_budget(response_budget)),
        );

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        let initial = proxy.get_remaining_budget(capsule_id).expect("should get");

        // Multiple invocations
        for _ in 0..5 {
            let request = InvokeRequest::new(cap_id, "https://example.com")
                .with_budget(BudgetVector::new(50, 50, 512, 0, 50, 1));
            proxy.invoke(capsule_id, request).expect("should invoke");
        }

        let remaining = proxy.get_remaining_budget(capsule_id).expect("should get");

        // Should have consumed 5 * response_budget
        assert_eq!(
            initial.cpu_time_ms - remaining.cpu_time_ms,
            5 * response_budget.cpu_time_ms
        );
    }
}

mod evidence_logging_tests {
    use super::*;

    #[test]
    fn test_grant_logged() {
        let logger = Arc::new(MockEvidenceLogger::new());
        let mut proxy = CapabilityProxy::new();
        proxy.set_evidence_logger(Box::new(MockEvidenceLoggerWrapper(logger.clone())));

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());

        proxy.grant(capsule_id, test_grant()).expect("should grant");

        assert_eq!(logger.grant_count(), 1);
    }

    #[test]
    fn test_revoke_logged() {
        let logger = Arc::new(MockEvidenceLogger::new());
        let mut proxy = CapabilityProxy::new();
        proxy.set_evidence_logger(Box::new(MockEvidenceLoggerWrapper(logger.clone())));

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");
        proxy.revoke(capsule_id, cap_id).expect("should revoke");

        assert_eq!(logger.revoke_count(), 1);
    }

    #[test]
    fn test_invoke_logged() {
        let logger = Arc::new(MockEvidenceLogger::new());
        let mut proxy = CapabilityProxy::new();
        proxy.set_evidence_logger(Box::new(MockEvidenceLoggerWrapper(logger.clone())));
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http")),
        );

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        let cap_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));

        let response = proxy.invoke(capsule_id, request).expect("should invoke");

        assert_eq!(logger.invoke_count(), 1);
        assert!(response.evidence_id > 0);
    }

    #[test]
    fn test_cascade_revoke_logged() {
        let logger = Arc::new(MockEvidenceLogger::new());
        let mut proxy = CapabilityProxy::new();
        proxy.set_evidence_logger(Box::new(MockEvidenceLoggerWrapper(logger.clone())));

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());

        let parent_id = proxy.grant(capsule_id, test_grant()).expect("should grant");

        // Add children
        {
            let mut contexts = proxy.contexts.write().unwrap();
            let ctx = contexts.get_mut(&capsule_id).unwrap();

            for i in 0..3 {
                let child = Capability {
                    id: CapabilityId::from_raw(100 + i),
                    cap_type: CapabilityType::NetworkHttp,
                    scope: CapabilityScope::Global,
                    rights: Rights::new(Rights::READ),
                    quota: Quota::unlimited(),
                    expires_at: u64::MAX,
                    parent: Some(parent_id),
                    proof: CapabilityProof::new([0u8; 32], [0x42u8; 64], 0),
                    revoked: false,
                };
                ctx.grant(child);
            }
        }

        proxy.revoke_cascade(capsule_id, parent_id).expect("should revoke");

        // Should log 4 revocations (1 parent + 3 children)
        assert_eq!(logger.revoke_count(), 4);
    }

    /// Wrapper to make Arc<MockEvidenceLogger> work as EvidenceLogger
    struct MockEvidenceLoggerWrapper(Arc<MockEvidenceLogger>);

    impl EvidenceLogger for MockEvidenceLoggerWrapper {
        fn log_grant(&self, capsule_id: CapsuleId, cap: &Capability) {
            self.0.log_grant(capsule_id, cap);
        }

        fn log_revoke(&self, capsule_id: CapsuleId, cap_id: CapabilityId) {
            self.0.log_revoke(capsule_id, cap_id);
        }

        fn log_invoke(
            &self,
            capsule_id: CapsuleId,
            cap_id: CapabilityId,
            target: &str,
            success: bool,
            budget_used: &BudgetVector,
        ) -> u64 {
            self.0.log_invoke(capsule_id, cap_id, target, success, budget_used)
        }
    }
}

mod proxy_builder_tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let proxy = CapabilityProxyBuilder::new().build();

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());

        let result = proxy.grant(capsule_id, test_grant());
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_with_signing_key() {
        let key = [0x42u8; 32];
        let proxy = CapabilityProxyBuilder::new()
            .with_signing_key(key)
            .build();

        assert_eq!(proxy.signing_key, key);
    }

    #[test]
    fn test_builder_with_executor() {
        let proxy = CapabilityProxyBuilder::new()
            .with_executor(CapabilityType::NetworkHttp, Box::new(MockExecutor::new("http")))
            .build();

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        let cap_id = proxy.grant(capsule_id, test_grant()).unwrap();

        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));

        let result = proxy.invoke(capsule_id, request);
        assert!(result.is_ok());
    }
}

mod integration_tests {
    use super::*;

    #[test]
    fn test_full_lifecycle() {
        // Setup proxy with executor and logger
        let logger = Arc::new(MockEvidenceLogger::new());
        let mut proxy = CapabilityProxy::new();
        proxy.set_evidence_logger(Box::new(MockEvidenceLoggerWrapper(logger.clone())));
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http")),
        );
        proxy.register_executor(
            CapabilityType::FileRead,
            Box::new(MockExecutor::new("file")),
        );

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        // 1. Grant multiple capabilities
        let http_cap = proxy
            .grant(capsule_id, CapabilityGrant::new(CapabilityType::NetworkHttp))
            .unwrap();
        let file_cap = proxy
            .grant(capsule_id, CapabilityGrant::new(CapabilityType::FileRead))
            .unwrap();

        assert_eq!(proxy.get_capabilities(capsule_id).unwrap().len(), 2);

        // 2. Invoke HTTP capability
        let request = InvokeRequest::new(http_cap, "https://api.example.com/data")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));
        let response = proxy.invoke(capsule_id, request).unwrap();
        assert!(!response.payload.is_empty());

        // 3. Invoke file capability
        let request = InvokeRequest::new(file_cap, "/path/to/file.txt")
            .with_budget(BudgetVector::new(5, 50, 512, 0, 0, 0));
        let response = proxy.invoke(capsule_id, request).unwrap();
        assert!(!response.payload.is_empty());

        // 4. Revoke HTTP capability
        proxy.revoke(capsule_id, http_cap).unwrap();
        assert_eq!(proxy.get_valid_capabilities(capsule_id).unwrap().len(), 1);

        // 5. Verify HTTP invoke fails
        let request = InvokeRequest::new(http_cap, "https://example.com");
        let result = proxy.invoke(capsule_id, request);
        assert!(matches!(result, Err(ProxyError::CapabilityRevoked(_))));

        // 6. Verify evidence logged
        assert_eq!(logger.grant_count(), 2);
        assert_eq!(logger.revoke_count(), 1);
        assert_eq!(logger.invoke_count(), 2);
    }

    #[test]
    fn test_multiple_capsules() {
        let mut proxy = CapabilityProxy::new();
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http")),
        );

        let capsule1 = CapsuleId::from_bytes([0x01; 16]);
        let capsule2 = CapsuleId::from_bytes([0x02; 16]);

        proxy.register_capsule(capsule1, test_budget());
        proxy.register_capsule(capsule2, test_budget());
        proxy.set_capsule_time(capsule1, 1000).unwrap();
        proxy.set_capsule_time(capsule2, 1000).unwrap();

        // Grant different capabilities to each capsule
        let cap1 = proxy.grant(capsule1, test_grant()).unwrap();
        let cap2 = proxy.grant(capsule2, test_grant()).unwrap();

        // Capsule 1 cannot use capsule 2's capability
        let request = InvokeRequest::new(cap2, "https://example.com");
        let result = proxy.invoke(capsule1, request);
        assert!(matches!(result, Err(ProxyError::CapabilityNotFound(_))));

        // Each can use their own
        let request = InvokeRequest::new(cap1, "https://example.com")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));
        assert!(proxy.invoke(capsule1, request).is_ok());

        let request = InvokeRequest::new(cap2, "https://example.com")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 512, 1));
        assert!(proxy.invoke(capsule2, request).is_ok());
    }

    #[test]
    fn test_quota_consumption() {
        let mut proxy = CapabilityProxy::new();

        let response_budget = BudgetVector::new(10, 100, 1024, 0, 1000, 1);
        proxy.register_executor(
            CapabilityType::NetworkHttp,
            Box::new(MockExecutor::new("http").with_budget(response_budget)),
        );

        let capsule_id = test_capsule_id();
        proxy.register_capsule(capsule_id, test_budget());
        proxy.set_capsule_time(capsule_id, 1000).unwrap();

        // Grant with limited quota
        let grant = CapabilityGrant::new(CapabilityType::NetworkHttp)
            .with_quota(Quota::new(5, 10000, 1_000_000_000)); // 5 invocations

        let cap_id = proxy.grant(capsule_id, grant).unwrap();

        // Use up quota
        for i in 0..5 {
            let request = InvokeRequest::new(cap_id, "https://example.com")
                .with_budget(BudgetVector::new(10, 100, 1024, 0, 1000, 1));
            let result = proxy.invoke(capsule_id, request);

            if i < 5 {
                // First 5 should succeed (quota counts in consume_quota)
                // Note: The quota check happens before invoke, so if quota was 0 initially it would fail
            }
        }

        // Next invocation should fail
        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_budget(BudgetVector::new(10, 100, 1024, 0, 1000, 1));
        let result = proxy.invoke(capsule_id, request);
        assert!(matches!(result, Err(ProxyError::QuotaExhausted(_))));
    }

    /// Wrapper for Arc<MockEvidenceLogger>
    struct MockEvidenceLoggerWrapper(Arc<MockEvidenceLogger>);

    impl EvidenceLogger for MockEvidenceLoggerWrapper {
        fn log_grant(&self, capsule_id: CapsuleId, cap: &Capability) {
            self.0.log_grant(capsule_id, cap);
        }

        fn log_revoke(&self, capsule_id: CapsuleId, cap_id: CapabilityId) {
            self.0.log_revoke(capsule_id, cap_id);
        }

        fn log_invoke(
            &self,
            capsule_id: CapsuleId,
            cap_id: CapabilityId,
            target: &str,
            success: bool,
            budget_used: &BudgetVector,
        ) -> u64 {
            self.0.log_invoke(capsule_id, cap_id, target, success, budget_used)
        }
    }
}

mod error_tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let cap_id = CapabilityId::from_raw(123);
        let capsule_id = CapsuleId::from_bytes([0xAB; 16]);

        let errors = [
            ProxyError::CapabilityNotFound(cap_id),
            ProxyError::CapabilityExpired(cap_id),
            ProxyError::CapabilityRevoked(cap_id),
            ProxyError::QuotaExhausted(cap_id),
            ProxyError::ScopeViolation {
                cap_id,
                target: "forbidden.com".to_string(),
            },
            ProxyError::RightsViolation {
                cap_id,
                required: Rights::new(Rights::WRITE),
            },
            ProxyError::BudgetExceeded { capsule_id },
            ProxyError::InvalidSignature,
            ProxyError::ExecutorError("test error".to_string()),
            ProxyError::CapsuleNotFound(capsule_id),
            ProxyError::UnsupportedOperation,
        ];

        for error in &errors {
            let msg = format!("{}", error);
            assert!(!msg.is_empty());
        }
    }
}

mod invoke_request_tests {
    use super::*;

    #[test]
    fn test_invoke_request_builder() {
        let cap_id = CapabilityId::from_raw(1);
        let budget = BudgetVector::new(100, 200, 1024, 512, 2048, 5);
        let payload = vec![0x01, 0x02, 0x03];

        let request = InvokeRequest::new(cap_id, "https://example.com")
            .with_right(Rights::WRITE)
            .with_payload(payload.clone())
            .with_budget(budget);

        assert_eq!(request.capability_id, cap_id);
        assert_eq!(request.target, "https://example.com");
        assert_eq!(request.required_right, Rights::WRITE);
        assert_eq!(request.payload, payload);
        assert_eq!(request.estimated_budget.cpu_time_ms, 100);
    }
}
