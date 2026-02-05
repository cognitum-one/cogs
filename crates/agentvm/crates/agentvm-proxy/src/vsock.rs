//! vsock communication for capability proxy.
//!
//! Provides communication channel between guest capsules and the host proxy
//! over virtio-vsock or TCP fallback for testing.

use crate::config::VsockConfig;
use crate::error::VsockError;
use crate::wire::{MessageCodec, MessageEnvelope, MessageType};
use bytes::BytesMut;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, error, info, warn};

/// CID for the host
pub const HOST_CID: u32 = 2;

/// Default vsock port for capability proxy
pub const DEFAULT_PORT: u32 = 5000;

/// Listener for incoming vsock connections
pub struct VsockListener {
    /// Configuration
    config: VsockConfig,
    /// TCP listener (fallback mode)
    tcp_listener: Option<TcpListener>,
    /// Connection semaphore for limiting concurrent connections
    semaphore: Arc<Semaphore>,
}

impl VsockListener {
    /// Create a new vsock listener
    pub async fn bind(config: &VsockConfig) -> Result<Self, VsockError> {
        let semaphore = Arc::new(Semaphore::new(config.max_connections));

        if config.tcp_fallback {
            // Use TCP for testing
            let addr: SocketAddr = config.tcp_address.parse().map_err(|e| {
                VsockError::ListenerError(format!("Invalid TCP address: {}", e))
            })?;

            let listener = TcpListener::bind(addr).await.map_err(|e| {
                VsockError::ListenerError(format!("Failed to bind TCP listener: {}", e))
            })?;

            info!("vsock proxy listening on TCP {} (fallback mode)", addr);

            Ok(Self {
                config: config.clone(),
                tcp_listener: Some(listener),
                semaphore,
            })
        } else {
            // Use actual vsock
            // Note: Real vsock implementation would use the vsock crate
            // For now, we'll just fail gracefully if not in TCP fallback mode
            warn!("vsock not available, please use tcp_fallback=true for testing");
            Err(VsockError::ListenerError(
                "vsock not available on this platform".to_string(),
            ))
        }
    }

    /// Accept an incoming connection
    pub async fn accept(&self) -> Result<VsockConnection, VsockError> {
        // Acquire permit for rate limiting
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| VsockError::ListenerError("Semaphore closed".to_string()))?;

        if let Some(ref listener) = self.tcp_listener {
            let (stream, addr) = listener.accept().await?;
            debug!("Accepted TCP connection from {}", addr);

            Ok(VsockConnection {
                config: self.config.clone(),
                stream: ConnectionStream::Tcp(stream),
                codec: MessageCodec::new(16 * 1024 * 1024),
                read_buffer: BytesMut::with_capacity(64 * 1024),
                _permit: permit,
            })
        } else {
            Err(VsockError::ListenerError("No listener available".to_string()))
        }
    }

    /// Get the listening address (for testing)
    pub fn local_addr(&self) -> Option<String> {
        if let Some(ref listener) = self.tcp_listener {
            listener.local_addr().ok().map(|a| a.to_string())
        } else {
            Some(format!("vsock://{}:{}", self.config.cid, self.config.port))
        }
    }
}

/// Connection stream type
enum ConnectionStream {
    /// TCP stream (fallback)
    Tcp(TcpStream),
    // Real vsock would be: Vsock(vsock::VsockStream),
}

impl ConnectionStream {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buf).await,
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.write_all(buf).await,
        }
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush().await,
        }
    }

    async fn shutdown(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.shutdown().await,
        }
    }
}

/// A connection from a guest capsule
pub struct VsockConnection {
    /// Configuration
    config: VsockConfig,
    /// Underlying stream
    stream: ConnectionStream,
    /// Message codec
    codec: MessageCodec,
    /// Read buffer
    read_buffer: BytesMut,
    /// Connection permit (for rate limiting)
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl VsockConnection {
    /// Receive a message from the guest
    pub async fn recv(&mut self) -> Result<MessageEnvelope, VsockError> {
        loop {
            // Try to decode from existing buffer
            if let Some(envelope) = self.codec.decode(&mut self.read_buffer)? {
                return Ok(envelope);
            }

            // Need more data
            let mut buf = [0u8; 8192];
            let timeout = tokio::time::timeout(self.config.read_timeout, self.stream.read(&mut buf));

            match timeout.await {
                Ok(Ok(0)) => return Err(VsockError::ConnectionClosed),
                Ok(Ok(n)) => {
                    self.read_buffer.extend_from_slice(&buf[..n]);
                }
                Ok(Err(e)) => return Err(VsockError::Io(e)),
                Err(_) => return Err(VsockError::Timeout),
            }
        }
    }

    /// Send a message to the guest
    pub async fn send(&mut self, envelope: &MessageEnvelope) -> Result<(), VsockError> {
        let mut buf = BytesMut::new();
        self.codec.encode(envelope, &mut buf);

        self.stream.write_all(&buf).await?;
        self.stream.flush().await?;

        Ok(())
    }

    /// Send an error response
    pub async fn send_error(
        &mut self,
        sequence: u64,
        cap_id: crate::types::CapabilityId,
        error: &crate::wire::ErrorPayload,
    ) -> Result<(), VsockError> {
        let envelope = MessageEnvelope::error_response(sequence, cap_id, error)
            .map_err(|e| VsockError::ParseError(crate::error::ParseError::InvalidPayload(e.to_string())))?;
        self.send(&envelope).await
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<(), VsockError> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

/// Connection handler that processes messages
pub struct ConnectionHandler {
    /// Connection
    conn: VsockConnection,
    /// Message sender
    message_tx: mpsc::Sender<(MessageEnvelope, mpsc::Sender<MessageEnvelope>)>,
}

impl ConnectionHandler {
    /// Create a new connection handler
    pub fn new(
        conn: VsockConnection,
        message_tx: mpsc::Sender<(MessageEnvelope, mpsc::Sender<MessageEnvelope>)>,
    ) -> Self {
        Self { conn, message_tx }
    }

    /// Run the connection handler loop
    pub async fn run(mut self) {
        loop {
            match self.conn.recv().await {
                Ok(envelope) => {
                    if matches!(envelope.message_type, MessageType::Shutdown) {
                        info!("Received shutdown request");
                        break;
                    }

                    if matches!(envelope.message_type, MessageType::Ping) {
                        let pong = MessageEnvelope::pong(envelope.sequence);
                        if let Err(e) = self.conn.send(&pong).await {
                            error!("Failed to send pong: {}", e);
                            break;
                        }
                        continue;
                    }

                    // Create response channel
                    let (response_tx, mut response_rx) = mpsc::channel(1);

                    // Send to proxy for processing
                    if self.message_tx.send((envelope, response_tx)).await.is_err() {
                        error!("Proxy channel closed");
                        break;
                    }

                    // Wait for response
                    if let Some(response) = response_rx.recv().await {
                        if let Err(e) = self.conn.send(&response).await {
                            error!("Failed to send response: {}", e);
                            break;
                        }
                    }
                }
                Err(VsockError::ConnectionClosed) => {
                    debug!("Connection closed by guest");
                    break;
                }
                Err(VsockError::Timeout) => {
                    debug!("Connection timed out");
                    break;
                }
                Err(e) => {
                    error!("Connection error: {}", e);
                    break;
                }
            }
        }

        let _ = self.conn.close().await;
    }
}

/// Client for connecting to a vsock proxy (for testing)
pub struct VsockClient {
    /// Connection
    conn: ConnectionStream,
    /// Codec
    codec: MessageCodec,
    /// Read buffer
    read_buffer: BytesMut,
    /// Next sequence number
    sequence: u64,
}

impl VsockClient {
    /// Connect to a proxy via TCP (for testing)
    pub async fn connect_tcp(addr: &str) -> Result<Self, VsockError> {
        let stream = TcpStream::connect(addr).await?;

        Ok(Self {
            conn: ConnectionStream::Tcp(stream),
            codec: MessageCodec::new(16 * 1024 * 1024),
            read_buffer: BytesMut::with_capacity(64 * 1024),
            sequence: 0,
        })
    }

    /// Send a request and wait for response
    pub async fn request(&mut self, envelope: MessageEnvelope) -> Result<MessageEnvelope, VsockError> {
        // Send request
        let mut buf = BytesMut::new();
        self.codec.encode(&envelope, &mut buf);
        self.conn.write_all(&buf).await?;
        self.conn.flush().await?;

        // Receive response
        loop {
            if let Some(response) = self.codec.decode(&mut self.read_buffer)? {
                return Ok(response);
            }

            let mut buf = [0u8; 8192];
            let n = self.conn.read(&mut buf).await?;
            if n == 0 {
                return Err(VsockError::ConnectionClosed);
            }
            self.read_buffer.extend_from_slice(&buf[..n]);
        }
    }

    /// Get next sequence number
    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.sequence;
        self.sequence += 1;
        seq
    }

    /// Send a ping
    pub async fn ping(&mut self) -> Result<(), VsockError> {
        let seq = self.next_sequence();
        let request = MessageEnvelope::ping(seq);
        let response = self.request(request).await?;

        if matches!(response.message_type, MessageType::Pong) {
            Ok(())
        } else {
            Err(VsockError::ParseError(crate::error::ParseError::InvalidMessageType(
                response.message_type as u16,
            )))
        }
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<(), VsockError> {
        self.conn.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_listener_fallback() {
        let config = VsockConfig {
            tcp_fallback: true,
            tcp_address: "127.0.0.1:0".to_string(), // OS assigns port
            ..Default::default()
        };

        let listener = VsockListener::bind(&config).await.unwrap();
        assert!(listener.local_addr().is_some());
    }

    #[tokio::test]
    async fn test_client_ping() {
        // Start a listener
        let config = VsockConfig {
            tcp_fallback: true,
            tcp_address: "127.0.0.1:0".to_string(),
            ..Default::default()
        };

        let listener = VsockListener::bind(&config).await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn acceptor task
        let accept_handle = tokio::spawn(async move {
            let mut conn = listener.accept().await.unwrap();

            // Receive ping
            let msg = conn.recv().await.unwrap();
            assert!(matches!(msg.message_type, MessageType::Ping));

            // Send pong
            let pong = MessageEnvelope::pong(msg.sequence);
            conn.send(&pong).await.unwrap();
        });

        // Connect client
        let mut client = VsockClient::connect_tcp(&addr).await.unwrap();

        // Send ping
        client.ping().await.unwrap();

        // Cleanup
        client.close().await.ok();
        accept_handle.await.ok();
    }
}
