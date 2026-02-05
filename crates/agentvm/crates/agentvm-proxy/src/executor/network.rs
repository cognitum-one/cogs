//! Network executor for HTTP capabilities.
//!
//! Handles HTTP/HTTPS requests with domain allowlist checking,
//! connection pooling, and rate limiting.

use crate::config::NetworkConfig;
use crate::error::ExecutorError;
use crate::executor::{Executor, ExecutorResult};
use crate::types::{Capability, CapabilityType, InvokeRequest, Operation, OperationResult, QuotaConsumed};
use async_trait::async_trait;
use governor::{Quota, RateLimiter};
use reqwest::{Client, Method, Url};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// HTTP network executor
pub struct HttpNetworkExecutor {
    /// HTTP client with connection pooling
    client: Client,
    /// Configuration
    config: NetworkConfig,
    /// Rate limiter (if enabled)
    rate_limiter: Option<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
}

impl HttpNetworkExecutor {
    /// Create a new HTTP network executor
    pub fn new(config: &NetworkConfig) -> Result<Self, ExecutorError> {
        let mut client_builder = Client::builder()
            .timeout(config.request_timeout)
            .user_agent(&config.user_agent)
            .gzip(true)
            .brotli(true)
            .redirect(reqwest::redirect::Policy::limited(10));

        if config.connection_pooling {
            client_builder = client_builder
                .pool_max_idle_per_host(config.max_idle_per_host);
        }

        let client = client_builder
            .build()
            .map_err(|e| ExecutorError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        let rate_limiter = if config.rate_limit_rps > 0 {
            let quota = Quota::per_second(
                NonZeroU32::new(config.rate_limit_rps).unwrap()
            );
            Some(RateLimiter::direct(quota))
        } else {
            None
        };

        Ok(Self {
            client,
            config: config.clone(),
            rate_limiter,
        })
    }

    /// Check if a domain is allowed
    fn is_domain_allowed(&self, url: &Url) -> bool {
        let host = match url.host_str() {
            Some(h) => h,
            None => return false,
        };

        // Check blocklist first (takes precedence)
        for pattern in &self.config.domain_blocklist {
            if Self::matches_pattern(pattern, host) {
                debug!("Domain {} blocked by pattern {}", host, pattern);
                return false;
            }
        }

        // If allowlist is empty, allow all (except blocked)
        if self.config.domain_allowlist.is_empty() {
            return true;
        }

        // Check allowlist
        for pattern in &self.config.domain_allowlist {
            if Self::matches_pattern(pattern, host) {
                return true;
            }
        }

        debug!("Domain {} not in allowlist", host);
        false
    }

    /// Check if a host matches a pattern (glob-style)
    fn matches_pattern(pattern: &str, host: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with("*.") {
            let suffix = &pattern[1..]; // ".example.com"
            return host.ends_with(suffix) || host == &pattern[2..];
        }

        host == pattern
    }

    /// Execute an HTTP request
    async fn execute_http_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Result<ExecutorResult, ExecutorError> {
        let start = Instant::now();

        // Parse URL
        let parsed_url = Url::parse(url)
            .map_err(|e| ExecutorError::Http(format!("Invalid URL: {}", e)))?;

        // Check domain allowlist
        if !self.is_domain_allowed(&parsed_url) {
            return Err(ExecutorError::PermissionDenied(format!(
                "Domain not allowed: {}",
                parsed_url.host_str().unwrap_or("unknown")
            )));
        }

        // Check rate limit
        if let Some(ref limiter) = self.rate_limiter {
            if limiter.check().is_err() {
                return Err(ExecutorError::RateLimited);
            }
        }

        // Parse method
        let method = Method::from_bytes(method.as_bytes())
            .map_err(|_| ExecutorError::Http(format!("Invalid HTTP method: {}", method)))?;

        // Build request
        let mut request_builder = self.client.request(method, parsed_url);

        // Add headers
        for (name, value) in headers {
            request_builder = request_builder.header(name.as_str(), value.as_str());
        }

        // Add body
        let request_size = if let Some(body) = body {
            if body.len() > self.config.max_request_size {
                return Err(ExecutorError::Http(format!(
                    "Request body too large: {} bytes (max {})",
                    body.len(),
                    self.config.max_request_size
                )));
            }
            request_builder = request_builder.body(body.to_vec());
            body.len()
        } else {
            0
        };

        // Execute request
        let response = request_builder.send().await?;

        let status = response.status().as_u16();
        let response_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Read response body with size limit
        let body = response.bytes().await?;
        if body.len() > self.config.max_response_size {
            return Err(ExecutorError::Http(format!(
                "Response body too large: {} bytes (max {})",
                body.len(),
                self.config.max_response_size
            )));
        }

        let elapsed = start.elapsed();
        let total_bytes = request_size + body.len();

        info!(
            "HTTP {} completed: status={}, bytes={}, duration={:?}",
            url, status, total_bytes, elapsed
        );

        Ok(ExecutorResult::new(
            OperationResult::HttpResponse {
                status,
                headers: response_headers,
                body: body.to_vec(),
            },
            QuotaConsumed::single(total_bytes as u64, elapsed.as_nanos() as u64),
        ))
    }

    /// Execute a TCP connection (stub)
    async fn execute_tcp_connect(
        &self,
        host: &str,
        port: u16,
    ) -> Result<ExecutorResult, ExecutorError> {
        // For now, just return an error - TCP connections would need more infrastructure
        Err(ExecutorError::NotSupported(format!(
            "TCP connections not yet implemented: {}:{}",
            host, port
        )))
    }

    /// Execute DNS resolution
    async fn execute_dns_resolve(&self, name: &str) -> Result<ExecutorResult, ExecutorError> {
        use tokio::net::lookup_host;

        let start = Instant::now();

        let addresses: Vec<String> = lookup_host(format!("{}:0", name))
            .await
            .map_err(|e| ExecutorError::Network(format!("DNS resolution failed: {}", e)))?
            .map(|addr| addr.ip().to_string())
            .collect();

        let elapsed = start.elapsed();

        Ok(ExecutorResult::new(
            OperationResult::DnsResolved { addresses },
            QuotaConsumed::single(name.len() as u64, elapsed.as_nanos() as u64),
        ))
    }
}

#[async_trait]
impl Executor for HttpNetworkExecutor {
    async fn execute(
        &self,
        capability: &Capability,
        request: &InvokeRequest,
    ) -> Result<ExecutorResult, ExecutorError> {
        match &request.operation {
            Operation::HttpRequest {
                method,
                url,
                headers,
                body,
            } => {
                // Verify capability allows this operation
                if !capability.scope.permits(&request.operation) {
                    return Err(ExecutorError::PermissionDenied(
                        "Capability scope does not permit this URL".to_string(),
                    ));
                }

                self.execute_http_request(method, url, headers, body.as_deref())
                    .await
            }
            Operation::TcpConnect { host, port } => {
                if capability.cap_type != CapabilityType::NetworkTcp {
                    return Err(ExecutorError::PermissionDenied(
                        "Capability does not allow TCP connections".to_string(),
                    ));
                }
                self.execute_tcp_connect(host, *port).await
            }
            Operation::DnsResolve { name } => {
                if capability.cap_type != CapabilityType::NetworkDns
                    && capability.cap_type != CapabilityType::NetworkHttp
                {
                    return Err(ExecutorError::PermissionDenied(
                        "Capability does not allow DNS resolution".to_string(),
                    ));
                }
                self.execute_dns_resolve(name).await
            }
            _ => Err(ExecutorError::NotSupported(format!(
                "Network executor cannot handle operation: {:?}",
                std::mem::discriminant(&request.operation)
            ))),
        }
    }

    fn can_handle(&self, capability: &Capability) -> bool {
        capability.cap_type.is_network()
    }

    fn name(&self) -> &'static str {
        "http-network"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_matching() {
        assert!(HttpNetworkExecutor::matches_pattern("*", "anything.com"));
        assert!(HttpNetworkExecutor::matches_pattern("*.github.com", "api.github.com"));
        assert!(HttpNetworkExecutor::matches_pattern("*.github.com", "github.com"));
        assert!(!HttpNetworkExecutor::matches_pattern("*.github.com", "evil.com"));
        assert!(HttpNetworkExecutor::matches_pattern("api.example.com", "api.example.com"));
        assert!(!HttpNetworkExecutor::matches_pattern("api.example.com", "other.example.com"));
    }

    #[test]
    fn test_allowlist_empty_allows_all() {
        let config = NetworkConfig {
            domain_allowlist: vec![],
            domain_blocklist: vec![],
            ..Default::default()
        };
        let executor = HttpNetworkExecutor::new(&config).unwrap();

        let url = Url::parse("https://any-domain.com/path").unwrap();
        assert!(executor.is_domain_allowed(&url));
    }

    #[test]
    fn test_blocklist_takes_precedence() {
        let config = NetworkConfig {
            domain_allowlist: vec!["*.example.com".to_string()],
            domain_blocklist: vec!["blocked.example.com".to_string()],
            ..Default::default()
        };
        let executor = HttpNetworkExecutor::new(&config).unwrap();

        let allowed_url = Url::parse("https://api.example.com/path").unwrap();
        let blocked_url = Url::parse("https://blocked.example.com/path").unwrap();

        assert!(executor.is_domain_allowed(&allowed_url));
        assert!(!executor.is_domain_allowed(&blocked_url));
    }
}
