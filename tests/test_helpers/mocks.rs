//! Mock factory utilities

use mockall::mock;

/// Create a mock database that returns success
pub fn mock_db_success() -> MockDatabase {
    let mut mock = MockDatabase::new();
    mock.expect_query()
        .returning(|_| Ok(vec![]));
    mock
}

/// Create a mock database that returns errors
pub fn mock_db_error() -> MockDatabase {
    let mut mock = MockDatabase::new();
    mock.expect_query()
        .returning(|_| Err(DbError::ConnectionFailed));
    mock
}

mock! {
    pub Database {
        fn query(&self, sql: &str) -> Result<Vec<Row>, DbError>;
        fn execute(&self, sql: &str) -> Result<u64, DbError>;
    }
}

#[derive(Debug)]
pub struct Row;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Connection failed")]
    ConnectionFailed,
    #[error("Query failed")]
    QueryFailed,
}

/// Create a mock HTTP client
pub fn mock_http_client_success() -> MockHttpClient {
    let mut mock = MockHttpClient::new();
    mock.expect_get()
        .returning(|_| Ok(Response { status: 200, body: vec![] }));
    mock
}

mock! {
    pub HttpClient {
        fn get(&self, url: &str) -> Result<Response, HttpError>;
        fn post(&self, url: &str, body: &[u8]) -> Result<Response, HttpError>;
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("Network error")]
    NetworkError,
    #[error("Timeout")]
    Timeout,
}

/// Create a mock cache that always hits
pub fn mock_cache_hit() -> MockCache {
    let mut mock = MockCache::new();
    mock.expect_get()
        .returning(|_| Some(vec![1, 2, 3]));
    mock
}

/// Create a mock cache that always misses
pub fn mock_cache_miss() -> MockCache {
    let mut mock = MockCache::new();
    mock.expect_get()
        .returning(|_| None);
    mock
}

mock! {
    pub Cache {
        fn get(&self, key: &str) -> Option<Vec<u8>>;
        fn set(&self, key: &str, value: Vec<u8>) -> Result<(), CacheError>;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Cache full")]
    CacheFull,
}
