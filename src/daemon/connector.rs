//! Daemon connector for spawning and connecting to the daemon process

use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use super::ipc::{IpcClient, IpcResponse, ProfileStatus};

/// Connection state to the daemon
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonConnectionState {
    /// Not connected to daemon
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Connected to daemon
    Connected,
    /// Daemon unavailable (not running or failed to connect)
    Unavailable,
}

impl std::fmt::Display for DaemonConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonConnectionState::Disconnected => write!(f, "Disconnected"),
            DaemonConnectionState::Connecting => write!(f, "Connecting..."),
            DaemonConnectionState::Connected => write!(f, "Connected"),
            DaemonConnectionState::Unavailable => write!(f, "Unavailable"),
        }
    }
}

/// Daemon connector for managing the daemon process
pub struct DaemonConnector {
    client: Option<IpcClient>,
}

impl DaemonConnector {
    /// Create a new daemon connector
    pub fn new() -> Self {
        Self { client: None }
    }

    /// Check if daemon is available
    pub async fn is_daemon_available() -> bool {
        IpcClient::is_daemon_available().await
    }

    /// Connect to the daemon
    pub async fn connect(&mut self) -> Result<(), String> {
        match IpcClient::connect().await {
            Ok(client) => {
                self.client = Some(client);
                Ok(())
            }
            Err(e) => Err(format!("Failed to connect to daemon: {}", e)),
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    /// Disconnect from daemon
    pub fn disconnect(&mut self) {
        self.client = None;
    }

    /// Spawn the daemon process
    pub async fn spawn_daemon() -> Result<(), String> {
        // Check if daemon is already running
        if Self::is_daemon_available().await {
            info!("Daemon is already running");
            return Ok(());
        }

        // Get the current executable path
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?;

        info!("Spawning daemon process: {:?}", exe_path);

        // Spawn the daemon process in the background
        let child = Command::new(&exe_path)
            .arg("run")
            .arg("--daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn daemon: {}", e))?;

        debug!("Daemon process spawned with PID: {}", child.id());

        // Wait for daemon to be ready (with timeout)
        for i in 0..20 {
            sleep(Duration::from_millis(100)).await;
            if Self::is_daemon_available().await {
                info!("Daemon is ready after {}ms", (i + 1) * 100);
                return Ok(());
            }
        }

        Err("Daemon failed to start within timeout".to_string())
    }

    /// Ensure daemon is running, spawn if necessary
    pub async fn ensure_daemon_running(&mut self) -> Result<(), String> {
        if !Self::is_daemon_available().await {
            Self::spawn_daemon().await?;
        }
        self.connect().await
    }

    /// Ping the daemon
    pub async fn ping(&mut self) -> Result<bool, String> {
        let client = self.client.as_mut().ok_or("Not connected to daemon")?;
        client.ping().await.map_err(|e| {
            self.client = None; // Mark as disconnected on error
            format!("Ping failed: {}", e)
        })
    }

    /// Get daemon status
    pub async fn status(&mut self) -> Result<(bool, f64, Vec<ProfileStatus>), String> {
        let client = self.client.as_mut().ok_or("Not connected to daemon")?;
        match client.status().await {
            Ok(IpcResponse::Status {
                running,
                uptime_secs,
                profiles,
            }) => Ok((running, uptime_secs, profiles)),
            Ok(IpcResponse::Error { message }) => Err(message),
            Ok(_) => Err("Unexpected response".to_string()),
            Err(e) => {
                self.client = None;
                Err(format!("Status request failed: {}", e))
            }
        }
    }

    /// Start a profile via daemon
    pub async fn start_profile(&mut self, name: &str) -> Result<String, String> {
        let client = self.client.as_mut().ok_or("Not connected to daemon")?;
        match client.start_profile(name).await {
            Ok(IpcResponse::Ok { message }) => Ok(message),
            Ok(IpcResponse::Error { message }) => Err(message),
            Ok(_) => Err("Unexpected response".to_string()),
            Err(e) => {
                self.client = None;
                Err(format!("Start profile request failed: {}", e))
            }
        }
    }

    /// Stop a profile via daemon
    pub async fn stop_profile(&mut self, name: &str) -> Result<String, String> {
        let client = self.client.as_mut().ok_or("Not connected to daemon")?;
        match client.stop_profile(name).await {
            Ok(IpcResponse::Ok { message }) => Ok(message),
            Ok(IpcResponse::Error { message }) => Err(message),
            Ok(_) => Err("Unexpected response".to_string()),
            Err(e) => {
                self.client = None;
                Err(format!("Stop profile request failed: {}", e))
            }
        }
    }

    /// List running profiles via daemon
    pub async fn list_running(&mut self) -> Result<Vec<ProfileStatus>, String> {
        let client = self.client.as_mut().ok_or("Not connected to daemon")?;
        client.list_running().await.map_err(|e| {
            self.client = None;
            format!("List running request failed: {}", e)
        })
    }

    /// Reload daemon configuration
    pub async fn reload_config(&mut self) -> Result<String, String> {
        let client = self.client.as_mut().ok_or("Not connected to daemon")?;
        match client.reload_config().await {
            Ok(IpcResponse::Ok { message }) => Ok(message),
            Ok(IpcResponse::Error { message }) => Err(message),
            Ok(_) => Err("Unexpected response".to_string()),
            Err(e) => {
                self.client = None;
                Err(format!("Reload config request failed: {}", e))
            }
        }
    }

    /// Request daemon shutdown
    pub async fn shutdown_daemon(&mut self) -> Result<String, String> {
        let client = self.client.as_mut().ok_or("Not connected to daemon")?;
        match client.shutdown().await {
            Ok(IpcResponse::Ok { message }) => {
                self.client = None;
                Ok(message)
            }
            Ok(IpcResponse::Error { message }) => Err(message),
            Ok(_) => Err("Unexpected response".to_string()),
            Err(e) => {
                self.client = None;
                Err(format!("Shutdown request failed: {}", e))
            }
        }
    }
}

impl Default for DaemonConnector {
    fn default() -> Self {
        Self::new()
    }
}
