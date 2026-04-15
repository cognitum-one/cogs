//! Comprehensive tests for agentvm-evidence crate
//!
//! Test coverage:
//! - Merkle tree append and root calculation
//! - Inclusion proof generation and verification
//! - Consistency proof verification
//! - Bundle signing and verification
//! - Property tests for Merkle tree

use super::*;
use alloc::vec;
use alloc::string::String;

mod merkle_tree_tests {
    use super::*;
    use crate::merkle::*;

    #[test]
    fn test_empty_tree() {
        let tree = MerkleTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.root(), [0u8; 32]);
    }

    #[test]
    fn test_single_leaf() {
        let mut tree = MerkleTree::new();
        let leaf = sha256(b"leaf1");

        let root = tree.append(leaf);

        assert_eq!(tree.len(), 1);
        assert!(!tree.is_empty());
        assert_eq!(root, leaf); // Single leaf IS the root
    }

    #[test]
    fn test_two_leaves() {
        let mut tree = MerkleTree::new();
        let leaf1 = sha256(b"leaf1");
        let leaf2 = sha256(b"leaf2");

        tree.append(leaf1);
        let root = tree.append(leaf2);

        assert_eq!(tree.len(), 2);
        assert_eq!(root, hash_pair(&leaf1, &leaf2));
    }

    #[test]
    fn test_three_leaves() {
        let mut tree = MerkleTree::new();
        let leaf1 = sha256(b"leaf1");
        let leaf2 = sha256(b"leaf2");
        let leaf3 = sha256(b"leaf3");

        tree.append(leaf1);
        tree.append(leaf2);
        let root = tree.append(leaf3);

        // Tree structure:
        //       root
        //      /    \
        //    h12    leaf3
        //   /  \
        // leaf1 leaf2

        let h12 = hash_pair(&leaf1, &leaf2);
        let expected_root = hash_pair(&h12, &leaf3);
        assert_eq!(root, expected_root);
    }

    #[test]
    fn test_four_leaves() {
        let mut tree = MerkleTree::new();
        let leaves: Vec<Hash> = (0..4).map(|i| sha256(&[i as u8])).collect();

        for leaf in &leaves {
            tree.append(*leaf);
        }

        // Tree structure:
        //        root
        //       /    \
        //     h01    h23
        //    /  \   /  \
        //   l0  l1 l2  l3

        let h01 = hash_pair(&leaves[0], &leaves[1]);
        let h23 = hash_pair(&leaves[2], &leaves[3]);
        let expected_root = hash_pair(&h01, &h23);

        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn test_from_leaves() {
        let leaves: Vec<Hash> = (0..5).map(|i| sha256(&[i as u8])).collect();

        let tree = MerkleTree::from_leaves(leaves.clone());

        assert_eq!(tree.len(), 5);

        // Verify we get the same root by appending
        let mut tree2 = MerkleTree::new();
        for leaf in &leaves {
            tree2.append(*leaf);
        }

        assert_eq!(tree.root(), tree2.root());
    }

    #[test]
    fn test_append_many() {
        let leaves: Vec<Hash> = (0..10).map(|i| sha256(&[i as u8])).collect();

        let mut tree = MerkleTree::new();
        tree.append_many(&leaves);

        assert_eq!(tree.len(), 10);

        // Verify same as individual appends
        let mut tree2 = MerkleTree::new();
        for leaf in &leaves {
            tree2.append(*leaf);
        }

        assert_eq!(tree.root(), tree2.root());
    }

    #[test]
    fn test_get_leaf() {
        let leaves: Vec<Hash> = (0..3).map(|i| sha256(&[i as u8])).collect();
        let tree = MerkleTree::from_leaves(leaves.clone());

        assert_eq!(tree.get_leaf(0), Some(&leaves[0]));
        assert_eq!(tree.get_leaf(1), Some(&leaves[1]));
        assert_eq!(tree.get_leaf(2), Some(&leaves[2]));
        assert_eq!(tree.get_leaf(3), None);
    }

    #[test]
    fn test_root_deterministic() {
        let leaves: Vec<Hash> = (0..8).map(|i| sha256(&[i as u8])).collect();

        let tree1 = MerkleTree::from_leaves(leaves.clone());
        let tree2 = MerkleTree::from_leaves(leaves.clone());

        assert_eq!(tree1.root(), tree2.root());
    }

    #[test]
    fn test_different_leaves_different_roots() {
        let leaves1: Vec<Hash> = (0..4).map(|i| sha256(&[i as u8])).collect();
        let leaves2: Vec<Hash> = (4..8).map(|i| sha256(&[i as u8])).collect();

        let tree1 = MerkleTree::from_leaves(leaves1);
        let tree2 = MerkleTree::from_leaves(leaves2);

        assert_ne!(tree1.root(), tree2.root());
    }
}

mod inclusion_proof_tests {
    use super::*;
    use crate::merkle::*;

    #[test]
    fn test_inclusion_proof_single_leaf() {
        let mut tree = MerkleTree::new();
        let leaf = sha256(b"only leaf");
        tree.append(leaf);

        let proof = tree.inclusion_proof(0).expect("should generate proof");

        assert_eq!(proof.leaf_index, 0);
        assert_eq!(proof.leaf_hash, leaf);
        assert_eq!(proof.tree_size, 1);
        assert!(proof.proof.is_empty()); // No siblings for single leaf
    }

    #[test]
    fn test_inclusion_proof_two_leaves() {
        let mut tree = MerkleTree::new();
        let leaf1 = sha256(b"leaf1");
        let leaf2 = sha256(b"leaf2");
        tree.append(leaf1);
        tree.append(leaf2);

        let root = tree.root();

        // Proof for first leaf
        let proof1 = tree.inclusion_proof(0).expect("should generate proof");
        assert!(proof1.verify(&root));
        assert_eq!(proof1.proof.len(), 1);
        assert_eq!(proof1.proof[0].hash, leaf2);

        // Proof for second leaf
        let proof2 = tree.inclusion_proof(1).expect("should generate proof");
        assert!(proof2.verify(&root));
        assert_eq!(proof2.proof[0].hash, leaf1);
    }

    #[test]
    fn test_inclusion_proof_four_leaves() {
        let leaves: Vec<Hash> = (0..4).map(|i| sha256(&[i as u8])).collect();
        let tree = MerkleTree::from_leaves(leaves.clone());
        let root = tree.root();

        // Verify proof for each leaf
        for i in 0..4 {
            let proof = tree.inclusion_proof(i).expect("should generate proof");
            assert!(proof.verify(&root), "proof for leaf {} should verify", i);
        }
    }

    #[test]
    fn test_inclusion_proof_eight_leaves() {
        let leaves: Vec<Hash> = (0..8).map(|i| sha256(&[i as u8])).collect();
        let tree = MerkleTree::from_leaves(leaves);
        let root = tree.root();

        for i in 0..8 {
            let proof = tree.inclusion_proof(i).expect("should generate proof");
            assert!(proof.verify(&root), "proof for leaf {} should verify", i);
            // Should have log2(8) = 3 elements
            assert!(proof.proof.len() <= 3);
        }
    }

    #[test]
    fn test_inclusion_proof_large_tree() {
        let leaves: Vec<Hash> = (0..100).map(|i| sha256(&[i as u8])).collect();
        let tree = MerkleTree::from_leaves(leaves);
        let root = tree.root();

        // Test a sampling of leaves
        for i in [0, 1, 49, 50, 99] {
            let proof = tree.inclusion_proof(i).expect("should generate proof");
            assert!(proof.verify(&root), "proof for leaf {} should verify", i);
        }
    }

    #[test]
    fn test_inclusion_proof_invalid_index() {
        let tree = MerkleTree::from_leaves(vec![sha256(b"a"), sha256(b"b")]);

        assert!(tree.inclusion_proof(2).is_none());
        assert!(tree.inclusion_proof(100).is_none());
    }

    #[test]
    fn test_inclusion_proof_wrong_root() {
        let tree = MerkleTree::from_leaves(vec![sha256(b"a"), sha256(b"b")]);
        let proof = tree.inclusion_proof(0).expect("should generate proof");

        let wrong_root = sha256(b"wrong root");
        assert!(!proof.verify(&wrong_root));
    }

    #[test]
    fn test_inclusion_proof_tampered_proof() {
        let tree = MerkleTree::from_leaves(vec![sha256(b"a"), sha256(b"b")]);
        let root = tree.root();
        let mut proof = tree.inclusion_proof(0).expect("should generate proof");

        // Tamper with the proof
        if !proof.proof.is_empty() {
            proof.proof[0].hash[0] ^= 0xFF;
        }

        assert!(!proof.verify(&root));
    }
}

mod consistency_proof_tests {
    use super::*;
    use crate::merkle::*;

    #[test]
    fn test_consistency_proof_basic() {
        let mut tree = MerkleTree::new();

        // Add initial leaves
        for i in 0..4 {
            tree.append(sha256(&[i as u8]));
        }
        let old_root = tree.root();
        let old_size = tree.len();

        // Add more leaves
        for i in 4..8 {
            tree.append(sha256(&[i as u8]));
        }
        let new_root = tree.root();

        // Generate consistency proof
        let proof = tree.consistency_proof(old_size).expect("should generate proof");

        assert_eq!(proof.old_size, old_size as u64);
        assert_eq!(proof.new_size, tree.len() as u64);
        assert_eq!(proof.old_root, old_root);
        assert_eq!(proof.new_root, new_root);
    }

    #[test]
    fn test_consistency_proof_verification() {
        let mut tree = MerkleTree::new();

        for i in 0..4 {
            tree.append(sha256(&[i as u8]));
        }
        let old_root = tree.root();
        let old_size = tree.len();

        for i in 4..8 {
            tree.append(sha256(&[i as u8]));
        }
        let new_root = tree.root();

        let proof = tree.consistency_proof(old_size).expect("should generate proof");

        assert!(proof.verify(&old_root, &new_root));
    }

    #[test]
    fn test_consistency_proof_wrong_old_root() {
        let mut tree = MerkleTree::new();
        for i in 0..8 {
            tree.append(sha256(&[i as u8]));
        }
        let new_root = tree.root();

        let proof = tree.consistency_proof(4).expect("should generate proof");

        let wrong_old_root = sha256(b"wrong");
        assert!(!proof.verify(&wrong_old_root, &new_root));
    }

    #[test]
    fn test_consistency_proof_invalid_size() {
        let tree = MerkleTree::from_leaves(vec![sha256(b"a"), sha256(b"b")]);

        assert!(tree.consistency_proof(0).is_none());
        assert!(tree.consistency_proof(3).is_none()); // Greater than tree size
    }
}

mod signing_tests {
    use super::*;
    use crate::sign::*;

    #[test]
    fn test_ed25519_signer_creation() {
        let signer = Ed25519Signer::new(
            "test-key".into(),
            [1u8; 32],
            [2u8; 32],
        );

        assert_eq!(signer.key_id(), "test-key");
        assert_eq!(signer.public_key(), &[2u8; 32]);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_ed25519_signer_generate() {
        let signer = Ed25519Signer::generate("generated-key".into());
        assert_eq!(signer.key_id(), "generated-key");
    }

    #[test]
    fn test_sign_and_verify() {
        let signer = Ed25519Signer::new(
            "test-key".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let data = b"data to sign";
        let signature = signer.sign(data);

        assert_eq!(signature.len(), 64);
        assert!(signer.verify(data, &signature));
    }

    #[test]
    fn test_verify_wrong_data() {
        let signer = Ed25519Signer::new(
            "test-key".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let data = b"data to sign";
        let signature = signer.sign(data);

        let wrong_data = b"different data";
        assert!(!signer.verify(wrong_data, &signature));
    }

    #[test]
    fn test_verify_wrong_signature() {
        let signer = Ed25519Signer::new(
            "test-key".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let data = b"data to sign";
        let wrong_signature = vec![0u8; 64];

        assert!(!signer.verify(data, &wrong_signature));
    }

    #[test]
    fn test_verify_invalid_signature_length() {
        let signer = Ed25519Signer::new(
            "test-key".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let data = b"data to sign";
        let short_signature = vec![0u8; 32]; // Too short

        assert!(!signer.verify(data, &short_signature));
    }

    #[test]
    fn test_sign_evidence() {
        let signer = Ed25519Signer::new(
            "capsule:abc123".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let data = b"evidence payload";
        let evidence_sig = sign_evidence(&signer, data);

        assert_eq!(evidence_sig.keyid, "capsule:abc123");
        assert_eq!(evidence_sig.sig.len(), 64);
    }

    #[test]
    fn test_verify_evidence_signature() {
        let signer = Ed25519Signer::new(
            "capsule:abc123".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let data = b"evidence payload";
        let evidence_sig = sign_evidence(&signer, data);

        assert!(verify_evidence_signature(&signer, data, &evidence_sig));
    }

    #[test]
    fn test_verify_evidence_signature_wrong_keyid() {
        let signer = Ed25519Signer::new(
            "capsule:abc123".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let data = b"evidence payload";
        let mut evidence_sig = sign_evidence(&signer, data);
        evidence_sig.keyid = "wrong-keyid".into();

        assert!(!verify_evidence_signature(&signer, data, &evidence_sig));
    }

    #[test]
    fn test_different_signers_different_signatures() {
        let signer1 = Ed25519Signer::new("key1".into(), [1u8; 32], [1u8; 32]);
        let signer2 = Ed25519Signer::new("key2".into(), [2u8; 32], [2u8; 32]);

        let data = b"same data";
        let sig1 = signer1.sign(data);
        let sig2 = signer2.sign(data);

        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_multi_signer() {
        let mut multi = MultiSigner::new(2); // Threshold of 2

        multi.add_signer(Box::new(Ed25519Signer::new("key1".into(), [1u8; 32], [1u8; 32])));
        multi.add_signer(Box::new(Ed25519Signer::new("key2".into(), [2u8; 32], [2u8; 32])));
        multi.add_signer(Box::new(Ed25519Signer::new("key3".into(), [3u8; 32], [3u8; 32])));

        let data = b"multi-signed data";
        let signatures = multi.sign_all(data);

        assert_eq!(signatures.len(), 3);

        // All signatures should verify
        assert!(multi.verify_threshold(data, &signatures));

        // Two signatures should also verify (threshold = 2)
        assert!(multi.verify_threshold(data, &signatures[..2]));

        // One signature should not verify
        assert!(!multi.verify_threshold(data, &signatures[..1]));
    }
}

mod bundle_tests {
    use super::*;
    use crate::bundle::*;
    use agentvm_types::{BudgetVector, CapsuleId, CapabilityType};
    use agentvm_types::evidence::NetworkDirection;

    fn create_test_capsule_id() -> CapsuleId {
        CapsuleId::from_bytes([1u8; 16])
    }

    #[test]
    fn test_evidence_builder_creation() {
        let capsule_id = create_test_capsule_id();
        let run_id = [2u8; 16];

        let builder = EvidenceBuilder::new(capsule_id, run_id);
        // Builder should be created successfully
        // (Internal state is private, so we test by building)
    }

    #[test]
    fn test_evidence_builder_with_inputs() {
        let capsule_id = create_test_capsule_id();
        let run_id = [2u8; 16];

        let builder = EvidenceBuilder::new(capsule_id, run_id)
            .manifest_hash([0xAA; 32])
            .workspace_hash([0xBB; 32])
            .environment_hash([0xCC; 32])
            .command(vec!["test".into(), "--arg".into()]);

        let chain = agentvm_types::evidence::EvidenceChain {
            sequence: 0,
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            inclusion_proof: vec![],
        };

        let statement = builder.build(0, [0xDD; 32], vec![], chain);

        assert_eq!(statement.inputs.manifest_hash, [0xAA; 32]);
        assert_eq!(statement.inputs.workspace_hash, [0xBB; 32]);
        assert_eq!(statement.inputs.environment_hash, Some([0xCC; 32]));
        assert_eq!(statement.inputs.command, vec!["test", "--arg"]);
    }

    #[test]
    fn test_evidence_builder_record_capability_call() {
        let capsule_id = create_test_capsule_id();
        let run_id = [2u8; 16];

        let mut builder = EvidenceBuilder::new(capsule_id, run_id);

        let record = capability_call_record(
            0,
            CapabilityType::NetworkHttp,
            12345,
            "GET",
            b"request",
            b"response",
            BudgetVector::new(10, 0, 0, 0, 100, 1),
            50000,
        );

        builder.record_capability_call(record);

        let chain = agentvm_types::evidence::EvidenceChain {
            sequence: 0,
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            inclusion_proof: vec![],
        };

        let statement = builder.build(0, [0u8; 32], vec![], chain);

        assert_eq!(statement.execution.capability_calls.len(), 1);
        assert_eq!(statement.execution.budget_consumed.cpu_time_ms, 10);
        assert_eq!(statement.execution.budget_consumed.network_bytes, 100);
    }

    #[test]
    fn test_evidence_builder_record_network_event() {
        let capsule_id = create_test_capsule_id();
        let run_id = [2u8; 16];

        let mut builder = EvidenceBuilder::new(capsule_id, run_id);

        let event = network_event_record(
            NetworkDirection::Egress,
            "api.example.com:443",
            1024,
            true,
        );

        builder.record_network_event(event);

        let chain = agentvm_types::evidence::EvidenceChain {
            sequence: 0,
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            inclusion_proof: vec![],
        };

        let statement = builder.build(0, [0u8; 32], vec![], chain);

        assert_eq!(statement.execution.network_events.len(), 1);
        assert_eq!(statement.execution.network_events[0].destination, "api.example.com:443");
    }

    #[test]
    fn test_evidence_logger() {
        let mut logger = EvidenceLogger::new();

        assert_eq!(logger.sequence(), 0);
        assert_eq!(logger.tree_size(), 0);

        // Create a test statement
        let header = agentvm_types::evidence::EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 12345,
            version: "1.0".into(),
            parent_run_id: None,
        };
        let statement = agentvm_types::evidence::EvidenceStatement::new(header);

        // Log it
        let chain = logger.log(&statement);

        assert_eq!(chain.sequence, 0);
        assert_eq!(chain.previous_hash, [0u8; 32]); // First entry
        assert_eq!(logger.sequence(), 1);
        assert_eq!(logger.tree_size(), 1);
    }

    #[test]
    fn test_evidence_logger_chain() {
        let mut logger = EvidenceLogger::new();

        // Log multiple statements
        for i in 0..5 {
            let header = agentvm_types::evidence::EvidenceHeader {
                run_id: [i as u8; 16],
                capsule_id: [2u8; 16],
                timestamp_ns: i as u64 * 1000,
                version: "1.0".into(),
                parent_run_id: None,
            };
            let statement = agentvm_types::evidence::EvidenceStatement::new(header);
            let chain = logger.log(&statement);

            assert_eq!(chain.sequence, i as u64);

            if i > 0 {
                // Should have non-zero previous hash after first
                assert_ne!(chain.previous_hash, [0u8; 32]);
            }
        }

        assert_eq!(logger.sequence(), 5);
        assert_eq!(logger.tree_size(), 5);
    }

    #[test]
    fn test_evidence_logger_inclusion_proof() {
        let mut logger = EvidenceLogger::new();

        // Add some statements
        for i in 0..4 {
            let header = agentvm_types::evidence::EvidenceHeader {
                run_id: [i as u8; 16],
                capsule_id: [2u8; 16],
                timestamp_ns: i as u64 * 1000,
                version: "1.0".into(),
                parent_run_id: None,
            };
            let statement = agentvm_types::evidence::EvidenceStatement::new(header);
            logger.log(&statement);
        }

        let root = logger.root();

        // Get and verify inclusion proofs
        for i in 0..4 {
            let proof = logger.inclusion_proof(i).expect("should have proof");
            assert!(proof.verify(&root));
        }
    }

    #[test]
    fn test_create_bundle() {
        let header = agentvm_types::evidence::EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 12345,
            version: "1.0".into(),
            parent_run_id: None,
        };
        let statement = agentvm_types::evidence::EvidenceStatement::new(header);

        let signatures = vec![
            agentvm_types::evidence::EvidenceSignature {
                keyid: "test-key".into(),
                sig: vec![0xAB; 64],
            }
        ];

        let bundle = create_bundle(&statement, signatures);

        assert_eq!(bundle.payload_type, "application/vnd.agentvm.evidence+json");
        assert_eq!(bundle.signatures.len(), 1);
    }
}

mod verification_tests {
    use super::*;
    use crate::verify::*;

    #[test]
    fn test_verify_bundle_valid() {
        let bundle = agentvm_types::evidence::EvidenceBundle {
            payload_type: "application/vnd.agentvm.evidence+json".into(),
            payload: "test payload".into(),
            signatures: vec![
                agentvm_types::evidence::EvidenceSignature {
                    keyid: "test".into(),
                    sig: vec![1, 2, 3],
                }
            ],
        };

        let result = verify_bundle(&bundle);

        assert!(result.valid_format);
        assert!(result.has_signatures);
        assert_eq!(result.signature_count, 1);
    }

    #[test]
    fn test_verify_bundle_invalid_payload_type() {
        let bundle = agentvm_types::evidence::EvidenceBundle {
            payload_type: "application/json".into(), // Wrong type
            payload: "test".into(),
            signatures: vec![],
        };

        let result = verify_bundle(&bundle);

        assert!(!result.valid_format);
        assert!(result.errors.iter().any(|e| e.contains("payload type")));
    }

    #[test]
    fn test_verify_bundle_no_signatures() {
        let bundle = agentvm_types::evidence::EvidenceBundle {
            payload_type: "application/vnd.agentvm.evidence+json".into(),
            payload: "test".into(),
            signatures: vec![],
        };

        let result = verify_bundle(&bundle);

        assert!(!result.has_signatures);
        assert!(result.errors.iter().any(|e| e.contains("no signatures")));
    }

    #[test]
    fn test_verify_bundle_empty_payload() {
        let bundle = agentvm_types::evidence::EvidenceBundle {
            payload_type: "application/vnd.agentvm.evidence+json".into(),
            payload: String::new(),
            signatures: vec![],
        };

        let result = verify_bundle(&bundle);

        assert!(!result.valid_format);
        assert!(result.errors.iter().any(|e| e.contains("empty payload")));
    }

    #[test]
    fn test_verify_chain_first_statement() {
        let header = agentvm_types::evidence::EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 12345,
            version: "1.0".into(),
            parent_run_id: None,
        };
        let mut statement = agentvm_types::evidence::EvidenceStatement::new(header);
        statement.chain.sequence = 0;
        statement.chain.previous_hash = [0u8; 32];

        let result = verify_chain(&statement, None, None);

        assert!(result.previous_valid);
    }

    #[test]
    fn test_verify_chain_with_expected_previous() {
        let header = agentvm_types::evidence::EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 12345,
            version: "1.0".into(),
            parent_run_id: None,
        };
        let mut statement = agentvm_types::evidence::EvidenceStatement::new(header);
        statement.chain.sequence = 1;
        statement.chain.previous_hash = [0xAB; 32];

        let expected = [0xAB; 32];
        let result = verify_chain(&statement, Some(&expected), None);

        assert!(result.previous_valid);
    }

    #[test]
    fn test_verify_chain_wrong_previous() {
        let header = agentvm_types::evidence::EvidenceHeader {
            run_id: [1u8; 16],
            capsule_id: [2u8; 16],
            timestamp_ns: 12345,
            version: "1.0".into(),
            parent_run_id: None,
        };
        let mut statement = agentvm_types::evidence::EvidenceStatement::new(header);
        statement.chain.sequence = 1;
        statement.chain.previous_hash = [0xAB; 32];

        let expected = [0xCD; 32]; // Different!
        let result = verify_chain(&statement, Some(&expected), None);

        assert!(!result.previous_valid);
        assert!(result.errors.iter().any(|e| e.contains("previous hash")));
    }

    #[test]
    fn test_verify_replay_matching() {
        let create_statement = || {
            let header = agentvm_types::evidence::EvidenceHeader {
                run_id: [1u8; 16],
                capsule_id: [2u8; 16],
                timestamp_ns: 12345,
                version: "1.0".into(),
                parent_run_id: None,
            };
            let mut statement = agentvm_types::evidence::EvidenceStatement::new(header);
            statement.outputs.exit_code = 0;
            statement.outputs.workspace_diff_hash = [0xAB; 32];
            statement
        };

        let original = create_statement();
        let replay = create_statement();

        let result = verify_replay(&original, &replay);

        assert!(result.matches);
        assert!(result.mismatches.is_empty());
        assert!((result.confidence - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_verify_replay_exit_code_mismatch() {
        let create_statement = |exit_code: i32| {
            let header = agentvm_types::evidence::EvidenceHeader {
                run_id: [1u8; 16],
                capsule_id: [2u8; 16],
                timestamp_ns: 12345,
                version: "1.0".into(),
                parent_run_id: None,
            };
            let mut statement = agentvm_types::evidence::EvidenceStatement::new(header);
            statement.outputs.exit_code = exit_code;
            statement
        };

        let original = create_statement(0);
        let replay = create_statement(1); // Different exit code

        let result = verify_replay(&original, &replay);

        assert!(!result.matches);
        assert!(result.mismatches.iter().any(|m| matches!(m, ReplayMismatch::ExitCode { .. })));
    }

    #[test]
    fn test_verify_replay_workspace_mismatch() {
        let create_statement = |diff: [u8; 32]| {
            let header = agentvm_types::evidence::EvidenceHeader {
                run_id: [1u8; 16],
                capsule_id: [2u8; 16],
                timestamp_ns: 12345,
                version: "1.0".into(),
                parent_run_id: None,
            };
            let mut statement = agentvm_types::evidence::EvidenceStatement::new(header);
            statement.outputs.workspace_diff_hash = diff;
            statement
        };

        let original = create_statement([0xAA; 32]);
        let replay = create_statement([0xBB; 32]); // Different diff

        let result = verify_replay(&original, &replay);

        assert!(!result.matches);
        assert!(result.mismatches.iter().any(|m| matches!(m, ReplayMismatch::WorkspaceDiff)));
    }
}

// Property-based tests using manual implementation since proptest may not be available
mod property_tests {
    use super::*;

    /// Simple pseudo-random number generator for property tests
    struct SimpleRng {
        state: u64,
    }

    impl SimpleRng {
        fn new(seed: u64) -> Self {
            Self { state: seed }
        }

        fn next(&mut self) -> u64 {
            self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
            self.state
        }

        fn next_bytes(&mut self, len: usize) -> alloc::vec::Vec<u8> {
            (0..len).map(|_| self.next() as u8).collect()
        }
    }

    #[test]
    fn property_merkle_root_deterministic() {
        let mut rng = SimpleRng::new(12345);

        for _ in 0..10 {
            let num_leaves = (rng.next() % 20) as usize + 1;
            let leaves: alloc::vec::Vec<Hash> = (0..num_leaves)
                .map(|_| {
                    let bytes = rng.next_bytes(32);
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&bytes);
                    hash
                })
                .collect();

            let tree1 = MerkleTree::from_leaves(leaves.clone());
            let tree2 = MerkleTree::from_leaves(leaves);

            assert_eq!(tree1.root(), tree2.root());
        }
    }

    #[test]
    fn property_inclusion_proof_always_verifies() {
        let mut rng = SimpleRng::new(54321);

        for _ in 0..10 {
            let num_leaves = (rng.next() % 20) as usize + 1;
            let leaves: alloc::vec::Vec<Hash> = (0..num_leaves)
                .map(|_| {
                    let bytes = rng.next_bytes(32);
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&bytes);
                    hash
                })
                .collect();

            let tree = MerkleTree::from_leaves(leaves);
            let root = tree.root();

            for i in 0..tree.len() {
                let proof = tree.inclusion_proof(i).expect("should generate proof");
                assert!(proof.verify(&root), "proof should verify for index {}", i);
            }
        }
    }

    #[test]
    fn property_different_leaves_different_roots() {
        let mut rng = SimpleRng::new(98765);

        for _ in 0..10 {
            let num_leaves = (rng.next() % 10) as usize + 2;

            let leaves1: alloc::vec::Vec<Hash> = (0..num_leaves)
                .map(|_| {
                    let bytes = rng.next_bytes(32);
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&bytes);
                    hash
                })
                .collect();

            let leaves2: alloc::vec::Vec<Hash> = (0..num_leaves)
                .map(|_| {
                    let bytes = rng.next_bytes(32);
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&bytes);
                    hash
                })
                .collect();

            // Very unlikely to be equal
            if leaves1 != leaves2 {
                let tree1 = MerkleTree::from_leaves(leaves1);
                let tree2 = MerkleTree::from_leaves(leaves2);
                assert_ne!(tree1.root(), tree2.root());
            }
        }
    }

    #[test]
    fn property_append_increases_tree_size() {
        let mut rng = SimpleRng::new(11111);
        let mut tree = MerkleTree::new();

        for i in 0..20 {
            assert_eq!(tree.len(), i);

            let bytes = rng.next_bytes(32);
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes);

            tree.append(hash);
            assert_eq!(tree.len(), i + 1);
        }
    }

    #[test]
    fn property_signing_is_deterministic() {
        let signer = crate::sign::Ed25519Signer::new(
            "test".into(),
            [0xAB; 32],
            [0xCD; 32],
        );

        let mut rng = SimpleRng::new(22222);

        for _ in 0..10 {
            let data = rng.next_bytes(100);

            let sig1 = signer.sign(&data);
            let sig2 = signer.sign(&data);

            assert_eq!(sig1, sig2);
        }
    }
}
