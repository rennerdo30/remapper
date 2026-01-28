//! Error types for Remapper

use std::path::PathBuf;
use thiserror::Error;

/// Main error type for Remapper operations
#[derive(Error, Debug)]
pub enum RemapperError {
    /// Device not found at path or with given name
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Permission denied accessing device
    #[cfg(target_os = "linux")]
    #[error("Permission denied: {0}. Try running with sudo or add user to 'input' group")]
    PermissionDenied(String),

    #[cfg(target_os = "windows")]
    #[error("Permission denied: {0}. Try running as Administrator")]
    PermissionDenied(String),

    #[cfg(target_os = "macos")]
    #[error("Permission denied: {0}. Check Accessibility permissions in System Preferences")]
    PermissionDenied(String),

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Failed to grab exclusive access to device
    #[error("Failed to grab device: {0}")]
    GrabFailed(String),

    /// Failed to release device grab
    #[error("Failed to ungrab device: {0}")]
    UngrabFailed(String),

    /// Configuration file error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    /// Failed to parse configuration
    #[error("Failed to parse configuration: {0}")]
    ConfigParseError(String),

    /// Profile not found
    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    /// Profile already exists
    #[error("Profile already exists: {0}")]
    ProfileExists(String),

    /// Invalid mapping configuration
    #[error("Invalid mapping: {0}")]
    InvalidMapping(String),

    /// Virtual device creation failed
    #[error("Failed to create virtual device: {0}")]
    UInputCreationFailed(String),

    /// Event read error
    #[error("Failed to read event: {0}")]
    EventReadError(String),

    /// Event write error
    #[error("Failed to write event: {0}")]
    EventWriteError(String),

    /// Engine already running
    #[error("Engine already running: {0}")]
    EngineAlreadyRunning(String),

    /// Engine not running
    #[error("Engine not running: {0}")]
    EngineNotRunning(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Platform-specific library error
    #[error("Platform error: {0}")]
    PlatformError(String),

    /// System error with errno (Linux only)
    #[cfg(target_os = "linux")]
    #[error("System error: {0}")]
    SystemError(#[from] nix::Error),

    /// Channel send error
    #[error("Channel error: failed to send message")]
    ChannelError,

    /// Timeout waiting for operation
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Feature not supported
    #[error("Not supported: {0}")]
    NotSupported(String),
}

#[cfg(target_os = "linux")]
impl From<evdev::Error> for RemapperError {
    fn from(err: evdev::Error) -> Self {
        RemapperError::PlatformError(err.to_string())
    }
}

/// Result type alias for Remapper operations
pub type Result<T> = std::result::Result<T, RemapperError>;
