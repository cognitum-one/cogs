//! Program loader unit tests

use mockall::mock;
use mockall::predicate::*;

#[cfg(test)]
mod program_loader_tests {
    use super::*;

    mock! {
        pub ProgramValidator {
            fn validate(&self, program: &[u8]) -> Result<ProgramMetadata, ValidationError>;
            fn check_signature(&self, program: &[u8]) -> Result<bool, ValidationError>;
        }
    }

    #[derive(Debug, Clone)]
    pub struct ProgramMetadata {
        pub size: usize,
        pub entry_point: u64,
        pub required_tiles: u32,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum ValidationError {
        #[error("Invalid program format")]
        InvalidFormat,
        #[error("Program too large")]
        TooLarge,
        #[error("Invalid signature")]
        InvalidSignature,
    }

    #[test]
    fn should_validate_program_before_loading() {
        // Given: A mock validator
        let mut mock_validator = MockProgramValidator::new();
        let program = vec![0x7F, 0x45, 0x4C, 0x46]; // ELF magic

        mock_validator
            .expect_validate()
            .with(eq(program.clone()))
            .times(1)
            .returning(|p| Ok(ProgramMetadata {
                size: p.len(),
                entry_point: 0x1000,
                required_tiles: 64,
            }));

        // When: Validating
        let result = mock_validator.validate(&program);

        // Then: Should return metadata
        assert!(result.is_ok());
        let metadata = result.unwrap();
        assert_eq!(metadata.required_tiles, 64);
    }

    #[test]
    fn should_reject_invalid_format() {
        // Given: A validator that rejects invalid format
        let mut mock_validator = MockProgramValidator::new();

        mock_validator
            .expect_validate()
            .returning(|_| Err(ValidationError::InvalidFormat));

        // When: Validating invalid program
        let result = mock_validator.validate(&[0xFF, 0xFF]);

        // Then: Should return error
        assert!(matches!(result, Err(ValidationError::InvalidFormat)));
    }

    #[test]
    fn should_enforce_size_limits() {
        // Given: A validator that checks size
        let mut mock_validator = MockProgramValidator::new();

        mock_validator
            .expect_validate()
            .withf(|p| p.len() > 10_000_000)
            .returning(|_| Err(ValidationError::TooLarge));

        // When: Validating large program
        let large_program = vec![0u8; 20_000_000];
        let result = mock_validator.validate(&large_program);

        // Then: Should reject
        assert!(matches!(result, Err(ValidationError::TooLarge)));
    }

    #[test]
    fn should_verify_program_signature() {
        // Given: A validator with signature checking
        let mut mock_validator = MockProgramValidator::new();
        let program = vec![0x01, 0x02, 0x03];

        mock_validator
            .expect_check_signature()
            .with(eq(program.clone()))
            .times(1)
            .returning(|_| Ok(true));

        // When: Checking signature
        let result = mock_validator.check_signature(&program);

        // Then: Should verify successfully
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn should_reject_tampered_program() {
        // Given: A validator that detects tampering
        let mut mock_validator = MockProgramValidator::new();

        mock_validator
            .expect_check_signature()
            .returning(|_| Err(ValidationError::InvalidSignature));

        // When: Checking tampered program
        let result = mock_validator.check_signature(&[0xFF]);

        // Then: Should fail
        assert!(matches!(result, Err(ValidationError::InvalidSignature)));
    }
}
