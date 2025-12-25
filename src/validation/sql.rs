//! SQL and NoSQL Injection Prevention
//!
//! Provides parameterized query building and input sanitization to prevent
//! SQL injection attacks and NoSQL operator injection.

use std::fmt;

/// Query parameter types for safe parameterized queries
#[derive(Debug, Clone, PartialEq)]
pub enum QueryParam {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
}

impl QueryParam {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            QueryParam::String(s) => Some(s),
            _ => None,
        }
    }
}

impl fmt::Display for QueryParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryParam::String(s) => write!(f, "'{}'", s.replace('\'', "''")),
            QueryParam::Integer(i) => write!(f, "{}", i),
            QueryParam::Float(fl) => write!(f, "{}", fl),
            QueryParam::Boolean(b) => write!(f, "{}", b),
            QueryParam::Null => write!(f, "NULL"),
        }
    }
}

/// SQL Validator for preventing injection attacks
#[derive(Debug, Default)]
pub struct SqlValidator;

impl SqlValidator {
    pub fn new() -> Self {
        Self
    }

    /// Sanitize string input for SQL queries by escaping special characters
    pub fn sanitize_string(&self, input: &str) -> String {
        // Replace single quotes with doubled single quotes (SQL standard escaping)
        // Remove null bytes
        // Remove SQL comment markers
        input
            .replace('\0', "")
            .replace("--", "")
            .replace("/*", "")
            .replace("*/", "")
            .replace('\'', "''")
    }

    /// Check if input contains SQL injection patterns
    pub fn contains_sql_injection(&self, input: &str) -> bool {
        let input_lower = input.to_lowercase();

        // Common SQL injection patterns
        let patterns = [
            "' or '1'='1",
            "' or 1=1",
            "'; drop table",
            "'; delete from",
            "'; update",
            "'; insert into",
            "union select",
            "exec(",
            "execute(",
            "xp_",
            "sp_",
        ];

        patterns.iter().any(|pattern| input_lower.contains(pattern))
    }

    /// Validate and sanitize NoSQL filter input
    pub fn sanitize_nosql_filter(&self, filter: &str) -> Result<String, NoSqlError> {
        // Block dangerous NoSQL operators
        let dangerous_operators = ["$where", "$regex", "$expr", "$function", "$accumulator"];

        for op in dangerous_operators {
            if filter.contains(op) {
                return Err(NoSqlError::DangerousOperator(op.to_string()));
            }
        }

        // Check for JavaScript code injection in MongoDB
        if filter.contains("function") || filter.contains("=>") {
            return Err(NoSqlError::JavaScriptInjection);
        }

        Ok(filter.to_string())
    }
}

/// NoSQL specific errors
#[derive(Debug, Clone, PartialEq)]
pub enum NoSqlError {
    DangerousOperator(String),
    JavaScriptInjection,
}

impl fmt::Display for NoSqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoSqlError::DangerousOperator(op) => {
                write!(f, "Dangerous NoSQL operator detected: {}", op)
            }
            NoSqlError::JavaScriptInjection => {
                write!(f, "JavaScript injection attempt detected")
            }
        }
    }
}

impl std::error::Error for NoSqlError {}

/// Parameterized query builder for safe SQL construction
#[derive(Debug, Default)]
pub struct QueryBuilder {
    query: String,
    params: Vec<QueryParam>,
    param_count: usize,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            params: Vec::new(),
            param_count: 0,
        }
    }

    /// Add raw SQL text (should only contain structure, no user input)
    pub fn raw(&mut self, sql: &str) -> &mut Self {
        self.query.push_str(sql);
        self
    }

    /// Add a parameterized value and return the placeholder
    pub fn param(&mut self, value: QueryParam) -> &mut Self {
        self.param_count += 1;
        self.params.push(value);
        self.query.push_str(&format!("${}", self.param_count));
        self
    }

    /// Build the final query with placeholders
    pub fn build(&self) -> (String, Vec<QueryParam>) {
        (self.query.clone(), self.params.clone())
    }

    /// Get the number of parameters
    pub fn param_count(&self) -> usize {
        self.params.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_string_escapes_single_quotes() {
        let validator = SqlValidator::new();
        let input = "O'Reilly";
        let sanitized = validator.sanitize_string(input);
        assert_eq!(sanitized, "O''Reilly");
    }

    #[test]
    fn sanitize_string_removes_null_bytes() {
        let validator = SqlValidator::new();
        let input = "text\0injection";
        let sanitized = validator.sanitize_string(input);
        assert!(!sanitized.contains('\0'));
    }

    #[test]
    fn sanitize_string_removes_sql_comments() {
        let validator = SqlValidator::new();
        let input = "value -- comment";
        let sanitized = validator.sanitize_string(input);
        assert!(!sanitized.contains("--"));
    }

    #[test]
    fn detects_classic_sql_injection() {
        let validator = SqlValidator::new();

        assert!(validator.contains_sql_injection("' OR '1'='1"));
        assert!(validator.contains_sql_injection("'; DROP TABLE users; --"));
        assert!(validator.contains_sql_injection("' UNION SELECT * FROM"));
    }

    #[test]
    fn allows_safe_input() {
        let validator = SqlValidator::new();

        assert!(!validator.contains_sql_injection("john@example.com"));
        assert!(!validator.contains_sql_injection("user_123"));
        assert!(!validator.contains_sql_injection("Normal Text"));
    }

    #[test]
    fn blocks_nosql_where_operator() {
        let validator = SqlValidator::new();
        let malicious = r#"{"$where": "sleep(5000)"}"#;

        let result = validator.sanitize_nosql_filter(malicious);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NoSqlError::DangerousOperator(_)));
    }

    #[test]
    fn blocks_nosql_regex_operator() {
        let validator = SqlValidator::new();
        let malicious = r#"{"$regex": ".*"}"#;

        let result = validator.sanitize_nosql_filter(malicious);
        assert!(result.is_err());
    }

    #[test]
    fn blocks_javascript_injection() {
        let validator = SqlValidator::new();
        let malicious = r#"{"code": "function() { return true; }"}"#;

        let result = validator.sanitize_nosql_filter(malicious);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NoSqlError::JavaScriptInjection));
    }

    #[test]
    fn query_builder_uses_placeholders() {
        let mut builder = QueryBuilder::new();
        builder.raw("SELECT * FROM users WHERE name = ");
        builder.param(QueryParam::String("'; DROP TABLE users; --".to_string()));

        let (query, params) = builder.build();

        // Query should use placeholder, not raw value
        assert!(query.contains("$1"));
        assert!(!query.contains("DROP TABLE"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn query_builder_handles_multiple_params() {
        let mut builder = QueryBuilder::new();
        builder.raw("SELECT * FROM users WHERE name = ");
        builder.param(QueryParam::String("john".to_string()));
        builder.raw(" AND age = ");
        builder.param(QueryParam::Integer(25));

        let (query, params) = builder.build();

        assert!(query.contains("$1"));
        assert!(query.contains("$2"));
        assert_eq!(params.len(), 2);
    }
}
