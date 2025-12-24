//! API response formatting unit tests

#[cfg(test)]
mod response_tests {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ApiResponse<T> {
        pub success: bool,
        pub data: Option<T>,
        pub error: Option<String>,
        pub timestamp: i64,
    }

    impl<T> ApiResponse<T> {
        pub fn success(data: T) -> Self {
            Self {
                success: true,
                data: Some(data),
                error: None,
                timestamp: chrono::Utc::now().timestamp(),
            }
        }

        pub fn error(message: String) -> Self {
            Self {
                success: false,
                data: None,
                error: Some(message),
                timestamp: chrono::Utc::now().timestamp(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: String,
    }

    #[test]
    fn should_create_success_response() {
        let data = TestData {
            value: "test".to_string(),
        };

        let response = ApiResponse::success(data.clone());

        assert!(response.success);
        assert_eq!(response.data, Some(data));
        assert_eq!(response.error, None);
        assert!(response.timestamp > 0);
    }

    #[test]
    fn should_create_error_response() {
        let response: ApiResponse<TestData> = ApiResponse::error("Error message".to_string());

        assert!(!response.success);
        assert_eq!(response.data, None);
        assert_eq!(response.error, Some("Error message".to_string()));
        assert!(response.timestamp > 0);
    }

    #[test]
    fn should_serialize_to_json() {
        let data = TestData {
            value: "test".to_string(),
        };
        let response = ApiResponse::success(data);

        let json = serde_json::to_string(&response);
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("\"success\":true"));
        assert!(json_str.contains("\"value\":\"test\""));
    }

    #[test]
    fn should_deserialize_from_json() {
        let json = r#"{
            "success": true,
            "data": {"value": "test"},
            "error": null,
            "timestamp": 1234567890
        }"#;

        let response: Result<ApiResponse<TestData>, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert!(response.success);
        assert_eq!(response.data.unwrap().value, "test");
    }
}
