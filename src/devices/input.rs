//! Input device wrapper

use std::path::Path;
use evdev::Device;
use tokio::io::unix::AsyncFd;
use tracing::{debug, trace};

use crate::core::error::{RemapperError, Result};
use crate::core::events::InputEvent;

/// Information about an input device
#[derive(Debug, Clone)]
pub struct InputDeviceInfo {
    /// Device path (e.g., /dev/input/event5)
    pub path: String,
    /// Device name
    pub name: String,
    /// Physical path
    pub phys: Option<String>,
    /// Unique identifier
    pub uniq: Option<String>,
    /// Vendor ID
    pub vendor: u16,
    /// Product ID
    pub product: u16,
    /// Version
    pub version: u16,
}

/// Wrapper around evdev input device
pub struct InputDevice {
    /// The evdev device
    device: Device,
    /// Async file descriptor wrapper
    async_fd: Option<AsyncFd<std::os::unix::io::RawFd>>,
    /// Device info
    info: InputDeviceInfo,
    /// Whether device is grabbed
    grabbed: bool,
}

impl InputDevice {
    /// Open an input device by path
    pub async fn open(path: &Path) -> Result<Self> {
        let device = Device::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                RemapperError::PermissionDenied(path.display().to_string())
            } else {
                RemapperError::DeviceNotFound(path.display().to_string())
            }
        })?;

        let info = InputDeviceInfo {
            path: path.display().to_string(),
            name: device.name().unwrap_or("Unknown").to_string(),
            phys: device.physical_path().map(|s| s.to_string()),
            uniq: device.unique_name().map(|s| s.to_string()),
            vendor: device.input_id().vendor(),
            product: device.input_id().product(),
            version: device.input_id().version(),
        };

        debug!("Opened device: {} ({})", info.name, info.path);

        // Set up async I/O
        use std::os::unix::io::{AsRawFd, BorrowedFd};
        let raw_fd = device.as_raw_fd();

        // Set non-blocking mode. nix 0.30 requires an `AsFd` rather than a raw fd.
        // SAFETY: `device` owns the fd and outlives these `fcntl` calls.
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
        let flags = nix::fcntl::fcntl(borrowed_fd, nix::fcntl::FcntlArg::F_GETFL)?;
        let mut flags = nix::fcntl::OFlag::from_bits_truncate(flags);
        flags.insert(nix::fcntl::OFlag::O_NONBLOCK);
        nix::fcntl::fcntl(borrowed_fd, nix::fcntl::FcntlArg::F_SETFL(flags))?;

        let async_fd = Some(AsyncFd::new(raw_fd)?);

        Ok(Self {
            device,
            async_fd,
            info,
            grabbed: false,
        })
    }

    /// Get device info
    pub fn info(&self) -> &InputDeviceInfo {
        &self.info
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Get device path
    pub fn path(&self) -> &str {
        &self.info.path
    }

    /// Check if device is grabbed
    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    /// Grab exclusive access to the device
    pub async fn grab(&mut self) -> Result<()> {
        if self.grabbed {
            return Ok(());
        }

        self.device
            .grab()
            .map_err(|e| RemapperError::GrabFailed(e.to_string()))?;
        self.grabbed = true;
        debug!("Grabbed device: {}", self.info.name);
        Ok(())
    }

    /// Release exclusive access to the device
    pub async fn ungrab(&mut self) -> Result<()> {
        if !self.grabbed {
            return Ok(());
        }

        self.device
            .ungrab()
            .map_err(|e| RemapperError::UngrabFailed(e.to_string()))?;
        self.grabbed = false;
        debug!("Ungrabbed device: {}", self.info.name);
        Ok(())
    }

    /// Read an event from the device (non-blocking)
    pub async fn read_event(&mut self) -> Result<Option<InputEvent>> {
        let async_fd = self.async_fd.as_ref().ok_or_else(|| {
            RemapperError::EventReadError("Device not in async mode".to_string())
        })?;

        loop {
            // Wait for the device to be readable
            let mut guard = async_fd.readable().await?;

            // Try to read events
            match self.device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if let Some(input_event) = InputEvent::from_evdev(&event) {
                            trace!("Read event: {}", input_event);
                            return Ok(Some(input_event));
                        }
                    }
                    // All events were filtered, try again
                    guard.clear_ready();
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No events available
                    guard.clear_ready();
                    return Ok(None);
                }
                Err(e) => {
                    return Err(RemapperError::EventReadError(e.to_string()));
                }
            }
        }
    }

    /// Get supported event types
    pub fn supported_events(&self) -> Vec<evdev::EventType> {
        self.device.supported_events().iter().collect()
    }

    /// Get supported keys
    pub fn supported_keys(&self) -> Vec<evdev::Key> {
        self.device
            .supported_keys()
            .map(|keys| keys.iter().collect())
            .unwrap_or_default()
    }

    /// Get supported absolute axes
    pub fn supported_absolute_axes(&self) -> Vec<evdev::AbsoluteAxisType> {
        self.device
            .supported_absolute_axes()
            .map(|axes| axes.iter().collect())
            .unwrap_or_default()
    }

    /// Get supported relative axes
    pub fn supported_relative_axes(&self) -> Vec<evdev::RelativeAxisType> {
        self.device
            .supported_relative_axes()
            .map(|axes| axes.iter().collect())
            .unwrap_or_default()
    }

    /// Get absolute axis info
    pub fn abs_info(&self, axis: evdev::AbsoluteAxisType) -> Option<evdev::AbsInfo> {
        self.device.get_abs_state().ok().and_then(|state| {
            state
                .get(axis.0 as usize)
                .map(|info| evdev::AbsInfo::new(info.value, info.minimum, info.maximum, info.fuzz, info.flat, info.resolution))
        })
    }

    /// Get the underlying evdev Device reference
    pub fn device(&self) -> &Device {
        &self.device
    }
}

impl Drop for InputDevice {
    fn drop(&mut self) {
        if self.grabbed {
            let _ = self.device.ungrab();
        }
    }
}
