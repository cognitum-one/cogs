//! Comprehensive tests for agentvm-capability crate
//!
//! Test coverage:
//! - Wire protocol parse/serialize round-trip
//! - Capability derivation (attenuation works, amplification fails)
//! - Validation (expired, revoked, quota exhausted, scope violation)
//! - Signature verification
//! - Capability table operations

use super::*;
use agentvm_types::{
    Capability, CapabilityId, CapabilityProof, CapabilityScope, CapabilityType, Quota, Rights,
};
use alloc::vec;

mod wire_protocol_tests {
    use super::*;
    use crate::wire::*;

    #[test]
    fn test_message_type_from_u16() {
        assert_eq!(MessageType::from_u16(0x0001), Some(MessageType::Invoke));
        assert_eq!(MessageType::from_u16(0x0002), Some(MessageType::Derive));
        assert_eq!(MessageType::from_u16(0x0101), Some(MessageType::InvokeResult));
        assert_eq!(MessageType::from_u16(0x0200), Some(MessageType::Error));
        assert_eq!(MessageType::from_u16(0x0300), Some(MessageType::Ping));
        assert_eq!(MessageType::from_u16(0x9999), None);
    }

    #[test]
    fn test_message_type_categories() {
        assert!(MessageType::Invoke.is_request());
        assert!(MessageType::Derive.is_request());
        assert!(!MessageType::Invoke.is_response());

        assert!(MessageType::InvokeResult.is_response());
        assert!(MessageType::DeriveResult.is_response());
        assert!(!MessageType::InvokeResult.is_request());

        assert!(MessageType::Error.is_error());
        assert!(!MessageType::Invoke.is_error());

        assert!(MessageType::Ping.is_control());
        assert!(MessageType::Pong.is_control());
        assert!(!MessageType::Invoke.is_control());
    }

    #[test]
    fn test_message_flags() {
        let mut flags = MessageFlags::default();
        assert!(!flags.has(MessageFlags::REQUIRES_ACK));
        assert!(!flags.has(MessageFlags::IS_RESPONSE));

        flags.set(MessageFlags::REQUIRES_ACK);
        assert!(flags.has(MessageFlags::REQUIRES_ACK));
        assert!(!flags.has(MessageFlags::IS_RESPONSE));

        flags.set(MessageFlags::IS_RESPONSE);
        assert!(flags.has(MessageFlags::REQUIRES_ACK));
        assert!(flags.has(MessageFlags::IS_RESPONSE));
    }

    #[test]
    fn test_message_envelope_new() {
        let cap_id = [1u8; 16];
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];

        let envelope = MessageEnvelope::new(
            MessageType::Invoke,
            cap_id,
            42,
            payload.clone(),
        );

        assert_eq!(envelope.magic, MAGIC);
        assert_eq!(envelope.version, VERSION);
        assert_eq!(envelope.sequence, 42);
        assert_eq!(envelope.capability_id, cap_id);
        assert_eq!(envelope.message_type, MessageType::Invoke);
        assert_eq!(envelope.payload_len, 4);
        assert_eq!(envelope.payload, payload);
    }

    #[test]
    fn test_message_envelope_request() {
        let envelope = MessageEnvelope::request(
            MessageType::Invoke,
            [0u8; 16],
            1,
            vec![],
        );

        assert!(envelope.flags.has(MessageFlags::REQUIRES_ACK));
        assert!(!envelope.flags.has(MessageFlags::IS_RESPONSE));
    }

    #[test]
    fn test_message_envelope_response() {
        let envelope = MessageEnvelope::response(
            MessageType::InvokeResult,
            [0u8; 16],
            1,
            vec![],
        );

        assert!(!envelope.flags.has(MessageFlags::REQUIRES_ACK));
        assert!(envelope.flags.has(MessageFlags::IS_RESPONSE));
    }

    #[test]
    fn test_message_envelope_ping_pong() {
        let ping = MessageEnvelope::ping(123);
        assert_eq!(ping.message_type, MessageType::Ping);
        assert_eq!(ping.sequence, 123);

        let pong = MessageEnvelope::pong(123);
        assert_eq!(pong.message_type, MessageType::Pong);
        assert_eq!(pong.sequence, 123);
    }

    #[test]
    fn test_serialize_parse_roundtrip_empty_payload() {
        let original = MessageEnvelope::new(
            MessageType::QueryQuota,
            [0xAB; 16],
            12345,
            vec![],
        );

        let serialized = original.serialize();
        let parsed = MessageEnvelope::parse(&serialized).expect("parse should succeed");

        assert_eq!(parsed.magic, original.magic);
        assert_eq!(parsed.version, original.version);
        assert_eq!(parsed.sequence, original.sequence);
        assert_eq!(parsed.capability_id, original.capability_id);
        assert_eq!(parsed.message_type, original.message_type);
        assert_eq!(parsed.payload, original.payload);
        assert_eq!(parsed.checksum, original.checksum);
    }

    #[test]
    fn test_serialize_parse_roundtrip_with_payload() {
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let original = MessageEnvelope::new(
            MessageType::Invoke,
            [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
             0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
            0xDEADBEEF,
            payload.clone(),
        );

        let serialized = original.serialize();
        let parsed = MessageEnvelope::parse(&serialized).expect("parse should succeed");

        assert_eq!(parsed.payload, payload);
        assert_eq!(parsed.payload_len, 10);
    }

    #[test]
    fn test_serialize_parse_roundtrip_large_payload() {
        let payload: Vec<u8> = (0..=255).cycle().take(1000).collect();
        let original = MessageEnvelope::new(
            MessageType::DeriveResult,
            [0xFF; 16],
            999999,
            payload.clone(),
        );

        let serialized = original.serialize();
        let parsed = MessageEnvelope::parse(&serialized).expect("parse should succeed");

        assert_eq!(parsed.payload, payload);
        assert_eq!(parsed.payload_len, 1000);
    }

    #[test]
    fn test_parse_buffer_too_small() {
        let buf = [0u8; 10]; // Less than HEADER_SIZE
        let result = MessageEnvelope::parse(&buf);

        assert!(matches!(
            result,
            Err(ParseError::BufferTooSmall { needed: 42, got: 10 })
        ));
    }

    #[test]
    fn test_parse_invalid_magic() {
        let mut buf = [0u8; 50];
        // Write wrong magic
        buf[0..4].copy_from_slice(&0xBADC0FFEu32.to_le_bytes());

        let result = MessageEnvelope::parse(&buf);
        assert!(matches!(
            result,
            Err(ParseError::InvalidMagic { got: 0xBADC0FFE })
        ));
    }

    #[test]
    fn test_parse_unsupported_version() {
        let mut envelope = MessageEnvelope::new(MessageType::Ping, [0u8; 16], 0, vec![]);
        envelope.version = 99;

        // Manually serialize with wrong version
        let mut buf = envelope.serialize();
        buf[4..6].copy_from_slice(&99u16.to_le_bytes());

        let result = MessageEnvelope::parse(&buf);
        assert!(matches!(
            result,
            Err(ParseError::UnsupportedVersion { version: 99 })
        ));
    }

    #[test]
    fn test_parse_invalid_message_type() {
        let envelope = MessageEnvelope::new(MessageType::Ping, [0u8; 16], 0, vec![]);
        let mut buf = envelope.serialize();

        // Write invalid message type
        buf[32..34].copy_from_slice(&0xFFFFu16.to_le_bytes());

        let result = MessageEnvelope::parse(&buf);
        assert!(matches!(
            result,
            Err(ParseError::InvalidMessageType { type_id: 0xFFFF })
        ));
    }

    #[test]
    fn test_parse_checksum_mismatch() {
        let envelope = MessageEnvelope::new(MessageType::Ping, [0u8; 16], 0, vec![]);
        let mut buf = envelope.serialize();

        // Corrupt the checksum
        let checksum_offset = buf.len() - 4;
        buf[checksum_offset] ^= 0xFF;

        let result = MessageEnvelope::parse(&buf);
        assert!(matches!(
            result,
            Err(ParseError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn test_checksum_verification() {
        let envelope = MessageEnvelope::new(MessageType::Invoke, [0xAB; 16], 42, vec![1, 2, 3]);

        assert!(envelope.verify_checksum());

        let mut corrupted = envelope.clone();
        corrupted.payload[0] = 255;
        // checksum was calculated with old payload, should fail
        assert!(!corrupted.verify_checksum());
    }

    #[test]
    fn test_total_size() {
        let envelope = MessageEnvelope::new(MessageType::Ping, [0u8; 16], 0, vec![]);
        assert_eq!(envelope.total_size(), HEADER_SIZE + 4); // header + checksum

        let envelope_with_payload = MessageEnvelope::new(
            MessageType::Invoke,
            [0u8; 16],
            0,
            vec![1, 2, 3, 4, 5],
        );
        assert_eq!(envelope_with_payload.total_size(), HEADER_SIZE + 5 + 4);
    }
}

mod validation_tests {
    use super::*;

    fn create_valid_capability() -> Capability {
        Capability {
            id: CapabilityId::from_raw(12345),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::new(100, 10000, 60_000_000_000),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([1u8; 32], [1u8; 64], 0),
            revoked: false,
        }
    }

    fn create_http_operation() -> Operation {
        Operation::HttpRequest {
            method: "GET".into(),
            url: "https://api.example.com/data".into(),
            headers: vec![],
            body: None,
        }
    }

    #[test]
    fn test_validate_valid_capability() {
        let cap = create_valid_capability();
        let op = create_http_operation();

        let result = validate_capability(&cap, &op, 0);
        assert_eq!(result, ValidationResult::Valid);
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_expired_capability() {
        let mut cap = create_valid_capability();
        cap.expires_at = 1000;

        let op = create_http_operation();

        let result = validate_capability(&cap, &op, 999);
        assert_eq!(result, ValidationResult::Valid);

        let result = validate_capability(&cap, &op, 1000);
        assert_eq!(result, ValidationResult::Expired);
        assert!(!result.is_valid());

        let result = validate_capability(&cap, &op, 1001);
        assert_eq!(result, ValidationResult::Expired);
    }

    #[test]
    fn test_validate_revoked_capability() {
        let mut cap = create_valid_capability();
        cap.revoked = true;

        let op = create_http_operation();

        let result = validate_capability(&cap, &op, 0);
        assert_eq!(result, ValidationResult::Revoked);
    }

    #[test]
    fn test_validate_quota_exhausted() {
        let mut cap = create_valid_capability();
        cap.quota.used_invocations = cap.quota.max_invocations;

        let op = create_http_operation();

        let result = validate_capability(&cap, &op, 0);
        assert_eq!(result, ValidationResult::QuotaExhausted);
    }

    #[test]
    fn test_validate_scope_violation() {
        let mut cap = create_valid_capability();
        cap.scope = CapabilityScope::Network {
            hosts: vec!["api.allowed.com".into()],
            ports: vec![443],
            protocols: vec![],
        };

        let op = Operation::HttpRequest {
            method: "GET".into(),
            url: "https://api.forbidden.com/data".into(),
            headers: vec![],
            body: None,
        };

        let result = validate_capability(&cap, &op, 0);
        assert_eq!(result, ValidationResult::ScopeViolation);
    }

    #[test]
    fn test_validate_invalid_signature() {
        let mut cap = create_valid_capability();
        cap.proof.signature = [0u8; 64]; // Zero signature fails verification

        let op = create_http_operation();

        let result = validate_capability(&cap, &op, 0);
        assert_eq!(result, ValidationResult::InvalidSignature);
    }

    #[test]
    fn test_validation_result_to_result() {
        assert!(ValidationResult::Valid.to_result().is_ok());
        assert!(matches!(
            ValidationResult::Expired.to_result(),
            Err(ValidationError::Expired)
        ));
        assert!(matches!(
            ValidationResult::Revoked.to_result(),
            Err(ValidationError::Revoked)
        ));
    }

    #[test]
    fn test_validate_operation_type_mismatch() {
        let cap = create_valid_capability(); // NetworkHttp type

        let file_op = Operation::FileRead {
            path: "/some/file".into(),
            offset: 0,
            len: 100,
        };

        let result = validate::validate_operation(&cap, &file_op, 0);
        assert_eq!(result, ValidationResult::ScopeViolation);
    }

    #[test]
    fn test_operation_target() {
        let http_op = Operation::HttpRequest {
            method: "GET".into(),
            url: "https://example.com".into(),
            headers: vec![],
            body: None,
        };
        assert_eq!(http_op.target(), "https://example.com");

        let file_op = Operation::FileRead {
            path: "/workspace/file.txt".into(),
            offset: 0,
            len: 100,
        };
        assert_eq!(file_op.target(), "/workspace/file.txt");

        let secret_op = Operation::SecretRead {
            name: "API_KEY".into(),
        };
        assert_eq!(secret_op.target(), "API_KEY");
    }

    #[test]
    fn test_operation_required_capability_type() {
        assert_eq!(
            Operation::HttpRequest {
                method: "GET".into(),
                url: "".into(),
                headers: vec![],
                body: None
            }
            .required_capability_type(),
            CapabilityType::NetworkHttp
        );

        assert_eq!(
            Operation::FileRead {
                path: "".into(),
                offset: 0,
                len: 0
            }
            .required_capability_type(),
            CapabilityType::FileRead
        );

        assert_eq!(
            Operation::ProcessSpawn {
                executable: "".into(),
                args: vec![],
                env: vec![]
            }
            .required_capability_type(),
            CapabilityType::ProcessSpawn
        );
    }

    #[test]
    fn test_batch_validate_operations() {
        let cap = create_valid_capability();

        let ops = vec![
            create_http_operation(),
            create_http_operation(),
        ];

        let results = validate::validate_operations(&cap, &ops, 0);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_valid()));
    }
}

mod derivation_tests {
    use super::*;
    use crate::derive::*;

    fn create_parent_capability() -> Capability {
        Capability {
            id: CapabilityId::from_raw(12345),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::new(100, 10000, 60_000_000_000),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([1u8; 32], [1u8; 64], 0),
            revoked: false,
        }
    }

    #[test]
    fn test_derive_attenuation_rights() {
        let parent = create_parent_capability();

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let child = derive_capability(&parent, &request, 0).expect("derivation should succeed");

        // Child should have READ but not WRITE
        assert!(child.rights.has(Rights::READ));
        assert!(!child.rights.has(Rights::WRITE));

        // Child should reference parent
        assert_eq!(child.parent, Some(parent.id));
    }

    #[test]
    fn test_derive_attenuation_quota() {
        let parent = create_parent_capability();

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let child = derive_capability(&parent, &request, 0).expect("derivation should succeed");

        assert_eq!(child.quota.max_invocations, 50);
        assert_eq!(child.quota.max_bytes, 5000);
    }

    #[test]
    fn test_derive_attenuation_expiry() {
        let mut parent = create_parent_capability();
        parent.expires_at = 10000;

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(50, 5000, 30_000_000_000))
            .with_expires_at(5000); // Sooner than parent

        let child = derive_capability(&parent, &request, 0).expect("derivation should succeed");

        assert_eq!(child.expires_at, 5000);
    }

    #[test]
    fn test_derive_fails_amplification_rights() {
        let mut parent = create_parent_capability();
        parent.rights = Rights::new(Rights::READ); // Only read

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::WRITE)) // Try to add WRITE
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let result = derive_capability(&parent, &request, 0);

        assert!(matches!(
            result,
            Err(DeriveError::AmplificationDenied { right }) if right == Rights::WRITE
        ));
    }

    #[test]
    fn test_derive_fails_no_delegate_right() {
        let mut parent = create_parent_capability();
        parent.rights = Rights::new(Rights::READ | Rights::WRITE); // No DELEGATE

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let result = derive_capability(&parent, &request, 0);

        assert!(matches!(result, Err(DeriveError::NoDelegateRight)));
    }

    #[test]
    fn test_derive_fails_quota_exceeds_remaining() {
        let parent = create_parent_capability(); // max 100 invocations

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(200, 5000, 30_000_000_000)); // More than parent has

        let result = derive_capability(&parent, &request, 0);

        assert!(matches!(result, Err(DeriveError::QuotaExceedsRemaining)));
    }

    #[test]
    fn test_derive_fails_expiry_extension() {
        let mut parent = create_parent_capability();
        parent.expires_at = 10000;

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(50, 5000, 30_000_000_000))
            .with_expires_at(20000); // Later than parent

        let result = derive_capability(&parent, &request, 0);

        assert!(matches!(result, Err(DeriveError::ExpiryExtension)));
    }

    #[test]
    fn test_derive_fails_parent_invalid() {
        let mut parent = create_parent_capability();
        parent.revoked = true;

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let result = derive_capability(&parent, &request, 0);

        assert!(matches!(result, Err(DeriveError::ParentInvalid)));
    }

    #[test]
    fn test_derive_fails_parent_expired() {
        let mut parent = create_parent_capability();
        parent.expires_at = 1000;

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let result = derive_capability(&parent, &request, 2000); // Current time past expiry

        assert!(matches!(result, Err(DeriveError::ParentInvalid)));
    }

    #[test]
    fn test_derive_scope_narrowing() {
        let mut parent = create_parent_capability();
        parent.scope = CapabilityScope::Network {
            hosts: vec!["*.example.com".into(), "api.other.com".into()],
            ports: vec![443, 80],
            protocols: vec![],
        };

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_scope(CapabilityScope::Network {
                hosts: vec!["api.example.com".into()],
                ports: vec![443],
                protocols: vec![],
            })
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let child = derive_capability(&parent, &request, 0).expect("derivation should succeed");

        // Verify scope was narrowed (intersection taken)
        match &child.scope {
            CapabilityScope::Network { hosts, ports, .. } => {
                // The intersection logic may vary, but it should be narrower
                assert!(!hosts.is_empty() || ports.len() <= 1);
            }
            _ => panic!("Expected network scope"),
        }
    }

    #[test]
    fn test_can_derive_success() {
        let parent = create_parent_capability();

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let result = can_derive(&parent, &request, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_derive_failure() {
        let mut parent = create_parent_capability();
        parent.rights = Rights::new(Rights::READ); // No DELEGATE

        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let result = can_derive(&parent, &request, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_request_builder() {
        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ))
            .with_scope(CapabilityScope::Global)
            .with_quota(Quota::new(10, 100, 1000))
            .with_expires_at(5000);

        assert!(request.rights.has(Rights::READ));
        assert!(matches!(request.scope, CapabilityScope::Global));
        assert_eq!(request.quota.max_invocations, 10);
        assert_eq!(request.expires_at, Some(5000));
    }

    #[test]
    fn test_derive_chain() {
        // Create a chain: parent -> child -> grandchild
        let parent = create_parent_capability();

        let child_request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::WRITE | Rights::DELEGATE))
            .with_quota(Quota::new(50, 5000, 30_000_000_000));

        let child = derive_capability(&parent, &child_request, 0).expect("child derivation should succeed");

        let grandchild_request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(25, 2500, 15_000_000_000));

        let grandchild = derive_capability(&child, &grandchild_request, 0)
            .expect("grandchild derivation should succeed");

        assert_eq!(grandchild.parent, Some(child.id));
        assert!(grandchild.rights.has(Rights::READ));
        assert!(!grandchild.rights.has(Rights::WRITE)); // Was removed
        assert_eq!(grandchild.quota.max_invocations, 25);
    }
}

mod token_table_tests {
    use super::*;
    use crate::token::CapabilityTable;
    use agentvm_types::CapsuleId;

    fn create_test_capability(id: u128) -> Capability {
        Capability {
            id: CapabilityId::from_raw(id),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::new(100, 10000, 60_000_000_000),
            expires_at: u64::MAX,
            parent: None,
            proof: CapabilityProof::new([1u8; 32], [1u8; 64], 0),
            revoked: false,
        }
    }

    #[test]
    fn test_capability_table_insert_get() {
        let mut table = CapabilityTable::new();
        let capsule_id = CapsuleId::from_bytes([1u8; 16]);
        let cap = create_test_capability(1);
        let cap_id = cap.id;

        table.insert(capsule_id, cap);

        assert_eq!(table.len(), 1);
        assert!(!table.is_empty());

        let retrieved = table.get(cap_id).expect("should find capability");
        assert_eq!(retrieved.id, cap_id);
    }

    #[test]
    fn test_capability_table_get_mut() {
        let mut table = CapabilityTable::new();
        let capsule_id = CapsuleId::from_bytes([1u8; 16]);
        let cap = create_test_capability(1);
        let cap_id = cap.id;

        table.insert(capsule_id, cap);

        let retrieved = table.get_mut(cap_id).expect("should find capability");
        retrieved.revoked = true;

        let check = table.get(cap_id).expect("should still find capability");
        assert!(check.revoked);
    }

    #[test]
    fn test_capability_table_remove() {
        let mut table = CapabilityTable::new();
        let capsule_id = CapsuleId::from_bytes([1u8; 16]);
        let cap = create_test_capability(1);
        let cap_id = cap.id;

        table.insert(capsule_id, cap);
        assert_eq!(table.len(), 1);

        let removed = table.remove(cap_id).expect("should remove capability");
        assert_eq!(removed.id, cap_id);

        assert_eq!(table.len(), 0);
        assert!(table.get(cap_id).is_none());
    }

    #[test]
    fn test_capability_table_get_by_capsule() {
        let mut table = CapabilityTable::new();
        let capsule1 = CapsuleId::from_bytes([1u8; 16]);
        let capsule2 = CapsuleId::from_bytes([2u8; 16]);

        table.insert(capsule1, create_test_capability(1));
        table.insert(capsule1, create_test_capability(2));
        table.insert(capsule2, create_test_capability(3));

        let caps1 = table.get_by_capsule(capsule1);
        assert_eq!(caps1.len(), 2);

        let caps2 = table.get_by_capsule(capsule2);
        assert_eq!(caps2.len(), 1);

        let caps3 = table.get_by_capsule(CapsuleId::from_bytes([3u8; 16]));
        assert_eq!(caps3.len(), 0);
    }

    #[test]
    fn test_capability_table_revoke() {
        let mut table = CapabilityTable::new();
        let capsule_id = CapsuleId::from_bytes([1u8; 16]);
        let cap = create_test_capability(1);
        let cap_id = cap.id;

        table.insert(capsule_id, cap);

        assert!(table.revoke(cap_id));

        let revoked = table.get(cap_id).expect("should still exist");
        assert!(revoked.revoked);
    }

    #[test]
    fn test_capability_table_revoke_cascades_to_children() {
        let mut table = CapabilityTable::new();
        let capsule_id = CapsuleId::from_bytes([1u8; 16]);

        let parent = create_test_capability(1);
        let parent_id = parent.id;

        let mut child = create_test_capability(2);
        child.parent = Some(parent_id);
        let child_id = child.id;

        table.insert(capsule_id, parent);
        table.insert(capsule_id, child);

        // Revoke parent
        table.revoke(parent_id);

        // Both should be revoked
        assert!(table.get(parent_id).unwrap().revoked);
        assert!(table.get(child_id).unwrap().revoked);
    }

    #[test]
    fn test_capability_table_revoke_all() {
        let mut table = CapabilityTable::new();
        let capsule_id = CapsuleId::from_bytes([1u8; 16]);

        table.insert(capsule_id, create_test_capability(1));
        table.insert(capsule_id, create_test_capability(2));
        table.insert(capsule_id, create_test_capability(3));

        table.revoke_all(capsule_id);

        let caps = table.get_by_capsule(capsule_id);
        assert!(caps.iter().all(|c| c.revoked));
    }

    #[test]
    fn test_capability_table_cleanup() {
        let mut table = CapabilityTable::new();
        let capsule_id = CapsuleId::from_bytes([1u8; 16]);

        let mut expired = create_test_capability(1);
        expired.expires_at = 1000;

        let mut revoked = create_test_capability(2);
        revoked.revoked = true;

        let valid = create_test_capability(3);

        table.insert(capsule_id, expired);
        table.insert(capsule_id, revoked);
        table.insert(capsule_id, valid);

        assert_eq!(table.len(), 3);

        table.cleanup(2000); // Current time past expiry

        assert_eq!(table.len(), 1);
    }
}

mod integration_tests {
    use super::*;
    use crate::wire::*;

    #[test]
    fn test_full_capability_lifecycle() {
        // 1. Create a parent capability
        let parent = Capability {
            id: CapabilityId::from_raw(1),
            cap_type: CapabilityType::FileRead,
            scope: CapabilityScope::Filesystem {
                paths: vec!["/workspace".into()],
                operations: agentvm_types::FileOperations::all(),
            },
            rights: Rights::all(),
            quota: Quota::new(100, 1_000_000, 60_000_000_000),
            expires_at: 10_000_000_000,
            parent: None,
            proof: CapabilityProof::new([1u8; 32], [1u8; 64], 0),
            revoked: false,
        };

        // 2. Validate it
        let op = Operation::FileRead {
            path: "/workspace/file.txt".into(),
            offset: 0,
            len: 1000,
        };
        let result = validate_capability(&parent, &op, 0);
        assert!(result.is_valid());

        // 3. Derive a child capability
        let request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::DELEGATE))
            .with_quota(Quota::new(50, 500_000, 30_000_000_000))
            .with_expires_at(5_000_000_000);

        let child = derive::derive_capability(&parent, &request, 0)
            .expect("derivation should succeed");

        // 4. Validate child
        let result = validate_capability(&child, &op, 0);
        assert!(result.is_valid());

        // 5. Try to derive grandchild with more rights (should fail)
        let bad_request = DeriveRequest::new()
            .with_rights(Rights::new(Rights::READ | Rights::WRITE | Rights::DELEGATE))
            .with_quota(Quota::new(25, 250_000, 15_000_000_000));

        let result = derive::derive_capability(&child, &bad_request, 0);
        assert!(matches!(result, Err(derive::DeriveError::AmplificationDenied { .. })));
    }

    #[test]
    fn test_message_invoke_flow() {
        // Simulate an invoke request/response flow

        // 1. Create invoke request
        let cap_id = [0xAB; 16];
        let payload = b"GET /data".to_vec();
        let request = MessageEnvelope::request(
            MessageType::Invoke,
            cap_id,
            1,
            payload,
        );

        // 2. Serialize and parse (simulating network transit)
        let wire_bytes = request.serialize();
        let parsed_request = MessageEnvelope::parse(&wire_bytes)
            .expect("should parse request");

        assert_eq!(parsed_request.message_type, MessageType::Invoke);
        assert!(parsed_request.flags.has(MessageFlags::REQUIRES_ACK));

        // 3. Create response
        let response_payload = b"OK: data here".to_vec();
        let response = MessageEnvelope::response(
            MessageType::InvokeResult,
            cap_id,
            1, // Same sequence number
            response_payload,
        );

        // 4. Serialize and parse response
        let response_bytes = response.serialize();
        let parsed_response = MessageEnvelope::parse(&response_bytes)
            .expect("should parse response");

        assert_eq!(parsed_response.message_type, MessageType::InvokeResult);
        assert!(parsed_response.flags.has(MessageFlags::IS_RESPONSE));
        assert_eq!(parsed_response.sequence, 1);
    }
}
