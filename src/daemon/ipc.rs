//! IPC protocol for daemon communication
//!
//! Uses Unix domain sockets on Linux/macOS for communication between
//! the GUI and daemon process.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, info, warn};

use crate::core::EngineState;

/// Get the path to the daemon socket
pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    runtime_dir.join("remapper.sock")
}

/// IPC request from client to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Ping the daemon to check if it's alive
    Ping,
    /// Get daemon status
    Status,
    /// Start a profile by name
    StartProfile { name: String },
    /// Stop a profile by name
    StopProfile { name: String },
    /// Get list of running profiles
    ListRunning,
    /// Reload configuration
    ReloadConfig,
    /// Shutdown the daemon
    Shutdown,
}

/// IPC response from daemon to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Pong response to ping
    Pong,
    /// Status response
    Status {
        running: bool,
        uptime_secs: f64,
        profiles: Vec<ProfileStatus>,
    },
    /// Profile operation succeeded
    Ok { message: String },
    /// Profile operation failed
    Error { message: String },
    /// List of running profiles
    RunningProfiles { profiles: Vec<ProfileStatus> },
}

/// Status of a profile in the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStatus {
    pub name: String,
    pub state: EngineState,
    pub events_processed: u64,
    pub uptime_secs: f64,
}

/// IPC client for connecting to the daemon
pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    /// Connect to the daemon
    pub async fn connect() -> std::io::Result<Self> {
        let path = socket_path();
        debug!("Connecting to daemon at {:?}", path);
        let stream = UnixStream::connect(&path).await?;
        Ok(Self { stream })
    }

    /// Check if daemon is available (non-blocking check)
    pub async fn is_daemon_available() -> bool {
        Self::connect().await.is_ok()
    }

    /// Send a request and receive a response
    pub async fn request(&mut self, request: &IpcRequest) -> std::io::Result<IpcResponse> {
        // Serialize request
        let mut msg = serde_json::to_string(request)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        msg.push('\n');

        // Send request
        self.stream.write_all(msg.as_bytes()).await?;
        self.stream.flush().await?;

        // Read response
        let mut reader = BufReader::new(&mut self.stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        // Parse response
        serde_json::from_str(&line)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Ping the daemon
    pub async fn ping(&mut self) -> std::io::Result<bool> {
        match self.request(&IpcRequest::Ping).await? {
            IpcResponse::Pong => Ok(true),
            _ => Ok(false),
        }
    }

    /// Get daemon status
    pub async fn status(&mut self) -> std::io::Result<IpcResponse> {
        self.request(&IpcRequest::Status).await
    }

    /// Start a profile
    pub async fn start_profile(&mut self, name: &str) -> std::io::Result<IpcResponse> {
        self.request(&IpcRequest::StartProfile {
            name: name.to_string(),
        })
        .await
    }

    /// Stop a profile
    pub async fn stop_profile(&mut self, name: &str) -> std::io::Result<IpcResponse> {
        self.request(&IpcRequest::StopProfile {
            name: name.to_string(),
        })
        .await
    }

    /// List running profiles
    pub async fn list_running(&mut self) -> std::io::Result<Vec<ProfileStatus>> {
        match self.request(&IpcRequest::ListRunning).await? {
            IpcResponse::RunningProfiles { profiles } => Ok(profiles),
            IpcResponse::Error { message } => Err(std::io::Error::other(message)),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unexpected response",
            )),
        }
    }

    /// Reload daemon configuration
    pub async fn reload_config(&mut self) -> std::io::Result<IpcResponse> {
        self.request(&IpcRequest::ReloadConfig).await
    }

    /// Request daemon shutdown
    pub async fn shutdown(&mut self) -> std::io::Result<IpcResponse> {
        self.request(&IpcRequest::Shutdown).await
    }
}

/// IPC server for the daemon
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    /// Create a new IPC server
    pub async fn new() -> std::io::Result<Self> {
        let path = socket_path();

        // Remove existing socket if present
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        info!("Starting IPC server at {:?}", path);
        let listener = UnixListener::bind(&path)?;

        Ok(Self { listener })
    }

    /// Accept a new connection
    pub async fn accept(&self) -> std::io::Result<IpcConnection> {
        let (stream, _addr) = self.listener.accept().await?;
        debug!("Accepted IPC connection");
        Ok(IpcConnection { stream })
    }

    /// Get the socket path
    pub fn path(&self) -> PathBuf {
        socket_path()
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Clean up socket file
        let path = socket_path();
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                warn!("Failed to remove socket file: {}", e);
            }
        }
    }
}

/// A single IPC connection from a client
pub struct IpcConnection {
    stream: UnixStream,
}

impl IpcConnection {
    /// Read the next request from the client
    pub async fn read_request(&mut self) -> std::io::Result<Option<IpcRequest>> {
        let mut reader = BufReader::new(&mut self.stream);
        let mut line = String::new();

        match reader.read_line(&mut line).await {
            Ok(0) => Ok(None), // Connection closed
            Ok(_) => {
                let request: IpcRequest = serde_json::from_str(&line)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                Ok(Some(request))
            }
            Err(e) => Err(e),
        }
    }

    /// Send a response to the client
    pub async fn send_response(&mut self, response: &IpcResponse) -> std::io::Result<()> {
        let mut msg = serde_json::to_string(response)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        msg.push('\n');
        self.stream.write_all(msg.as_bytes()).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = IpcRequest::StartProfile {
            name: "test".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("start_profile"));
        assert!(json.contains("test"));

        let parsed: IpcRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcRequest::StartProfile { name } => assert_eq!(name, "test"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_response_serialization() {
        let resp = IpcResponse::Status {
            running: true,
            uptime_secs: 123.45,
            profiles: vec![ProfileStatus {
                name: "test".to_string(),
                state: EngineState::Running,
                events_processed: 1000,
                uptime_secs: 60.0,
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("status"));
        assert!(json.contains("running"));

        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcResponse::Status { running, .. } => assert!(running),
            _ => panic!("Wrong variant"),
        }
    }
}
