//! Linux input device implementation using evdev

use std::fs;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use evdev::Device;
use tokio::io::unix::AsyncFd;
use tracing::{debug, trace};

use crate::core::error::{RemapperError, Result};
use crate::platform::traits::{
    AbsAxisInfo, DeviceCapabilities, DeviceType, InputBackend, PlatformDeviceInfo,
    PlatformInputDevice, PlatformInputEvent,
};

/// Linux input backend using evdev
pub struct LinuxInputBackend;

impl LinuxInputBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputBackend for LinuxInputBackend {
    async fn list_devices(&self) -> Result<Vec<PlatformDeviceInfo>> {
        let input_dir = Path::new("/dev/input");
        let mut devices = Vec::new();

        let entries = fs::read_dir(input_dir)
            .map_err(|e| RemapperError::PermissionDenied(format!("/dev/input: {}", e)))?;

        for entry in entries.flatten() {
            let path = entry.path();

            // Only look at event* devices
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with("event") {
                    continue;
                }
            } else {
                continue;
            }

            trace!("Checking device: {}", path.display());

            match Device::open(&path) {
                Ok(device) => {
                    let info = device_to_platform_info(&path, &device);
                    debug!("Found device: {} ({})", info.name, info.id);
                    devices.push(info);
                }
                Err(e) => {
                    trace!("Could not open {}: {}", path.display(), e);
                }
            }
        }

        // Sort by path for consistent ordering
        devices.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(devices)
    }

    async fn open_device(&self, device_id: &str) -> Result<Box<dyn PlatformInputDevice>> {
        let path = PathBuf::from(device_id);
        let device = LinuxInputDevice::open(&path).await?;
        Ok(Box::new(device))
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<PlatformDeviceInfo>> {
        let devices = self.list_devices().await?;
        let name_lower = name.to_lowercase();
        Ok(devices
            .into_iter()
            .find(|d| d.name.to_lowercase().contains(&name_lower)))
    }

    async fn find_by_id(&self, vendor: u16, product: u16) -> Result<Option<PlatformDeviceInfo>> {
        let devices = self.list_devices().await?;
        Ok(devices
            .into_iter()
            .find(|d| d.vendor_id == vendor && d.product_id == product))
    }
}

/// Linux input device wrapper around evdev
pub struct LinuxInputDevice {
    /// The evdev device
    device: Device,
    /// Async file descriptor wrapper
    async_fd: Option<AsyncFd<std::os::unix::io::RawFd>>,
    /// Device info
    info: PlatformDeviceInfo,
    /// Whether device is grabbed
    grabbed: bool,
}

impl LinuxInputDevice {
    /// Open an input device by path
    pub async fn open(path: &Path) -> Result<Self> {
        let device = Device::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                RemapperError::PermissionDenied(path.display().to_string())
            } else {
                RemapperError::DeviceNotFound(path.display().to_string())
            }
        })?;

        let info = device_to_platform_info(path, &device);

        debug!("Opened device: {} ({})", info.name, info.id);

        // Set up async I/O
        let raw_fd = device.as_raw_fd();

        // Set non-blocking mode
        let flags = nix::fcntl::fcntl(raw_fd, nix::fcntl::FcntlArg::F_GETFL)?;
        let mut flags = nix::fcntl::OFlag::from_bits_truncate(flags);
        flags.insert(nix::fcntl::OFlag::O_NONBLOCK);
        nix::fcntl::fcntl(raw_fd, nix::fcntl::FcntlArg::F_SETFL(flags))?;

        let async_fd = Some(AsyncFd::new(raw_fd)?);

        Ok(Self {
            device,
            async_fd,
            info,
            grabbed: false,
        })
    }

    /// Get the underlying evdev Device reference
    pub fn evdev_device(&self) -> &Device {
        &self.device
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
        self.device
            .get_abs_state()
            .ok()
            .and_then(|state| state.get(axis.0 as usize).copied())
    }
}

#[async_trait]
impl PlatformInputDevice for LinuxInputDevice {
    async fn read_event(&mut self) -> Result<Option<PlatformInputEvent>> {
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
                        if let Some(event_type) = evdev_event_type_to_u16(event.event_type()) {
                            let platform_event = PlatformInputEvent {
                                event_type,
                                code: event.code(),
                                value: event.value(),
                                timestamp_us: (event.timestamp().tv_sec as u64 * 1_000_000)
                                    + (event.timestamp().tv_usec as u64),
                            };
                            trace!("Read event: {:?}", platform_event);
                            return Ok(Some(platform_event));
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

    async fn grab(&mut self) -> Result<()> {
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

    async fn ungrab(&mut self) -> Result<()> {
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

    fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    fn info(&self) -> &PlatformDeviceInfo {
        &self.info
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let keys: Vec<u16> = self
            .device
            .supported_keys()
            .map(|keys| keys.iter().map(|k| k.0).collect())
            .unwrap_or_default();

        let abs_axes: Vec<AbsAxisInfo> = self
            .device
            .supported_absolute_axes()
            .map(|axes| {
                axes.iter()
                    .filter_map(|axis| {
                        self.abs_info(axis).map(|info| AbsAxisInfo {
                            code: axis.0,
                            value: info.value,
                            minimum: info.minimum,
                            maximum: info.maximum,
                            fuzz: info.fuzz,
                            flat: info.flat,
                            resolution: info.resolution,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let rel_axes: Vec<u16> = self
            .device
            .supported_relative_axes()
            .map(|axes| axes.iter().map(|a| a.0).collect())
            .unwrap_or_default();

        DeviceCapabilities {
            keys,
            abs_axes,
            rel_axes,
        }
    }
}

impl Drop for LinuxInputDevice {
    fn drop(&mut self) {
        if self.grabbed {
            let _ = self.device.ungrab();
        }
    }
}

/// Convert evdev Device to PlatformDeviceInfo
fn device_to_platform_info(path: &Path, device: &Device) -> PlatformDeviceInfo {
    let id = device.input_id();

    // Detect device type based on capabilities
    let is_gamepad = device
        .supported_keys()
        .map(|keys| {
            keys.contains(evdev::Key::BTN_GAMEPAD)
                || keys.contains(evdev::Key::BTN_A)
                || keys.contains(evdev::Key::BTN_SOUTH)
        })
        .unwrap_or(false);

    let is_keyboard = device
        .supported_keys()
        .map(|keys| {
            keys.contains(evdev::Key::KEY_A)
                && keys.contains(evdev::Key::KEY_Z)
                && keys.contains(evdev::Key::KEY_ENTER)
        })
        .unwrap_or(false);

    let has_rel = device.supported_relative_axes().is_some();
    let is_mouse = has_rel
        && device
            .supported_keys()
            .map(|keys| keys.contains(evdev::Key::BTN_LEFT))
            .unwrap_or(false);

    let device_type = if is_gamepad {
        DeviceType::Gamepad
    } else if is_keyboard {
        DeviceType::Keyboard
    } else if is_mouse {
        DeviceType::Mouse
    } else {
        DeviceType::Other
    };

    PlatformDeviceInfo {
        id: path.display().to_string(),
        name: device.name().unwrap_or("Unknown").to_string(),
        vendor_id: id.vendor(),
        product_id: id.product(),
        device_type,
        path: Some(path.to_path_buf()),
        supports_grab: true,
    }
}

/// Convert evdev EventType to u16
fn evdev_event_type_to_u16(event_type: evdev::EventType) -> Option<u16> {
    match event_type {
        evdev::EventType::SYNCHRONIZATION => Some(0),
        evdev::EventType::KEY => Some(1),
        evdev::EventType::RELATIVE => Some(2),
        evdev::EventType::ABSOLUTE => Some(3),
        evdev::EventType::MISC => Some(4),
        evdev::EventType::SWITCH => Some(5),
        evdev::EventType::LED => Some(17),
        evdev::EventType::SOUND => Some(18),
        evdev::EventType::REPEAT => Some(20),
        evdev::EventType::FORCEFEEDBACK => Some(21),
        evdev::EventType::POWER => Some(22),
        evdev::EventType::FORCEFEEDBACKSTATUS => Some(23),
        _ => None,
    }
}

/// Convert u16 to evdev EventType
pub fn u16_to_evdev_event_type(value: u16) -> Option<evdev::EventType> {
    match value {
        0 => Some(evdev::EventType::SYNCHRONIZATION),
        1 => Some(evdev::EventType::KEY),
        2 => Some(evdev::EventType::RELATIVE),
        3 => Some(evdev::EventType::ABSOLUTE),
        4 => Some(evdev::EventType::MISC),
        5 => Some(evdev::EventType::SWITCH),
        17 => Some(evdev::EventType::LED),
        18 => Some(evdev::EventType::SOUND),
        20 => Some(evdev::EventType::REPEAT),
        21 => Some(evdev::EventType::FORCEFEEDBACK),
        22 => Some(evdev::EventType::POWER),
        23 => Some(evdev::EventType::FORCEFEEDBACKSTATUS),
        _ => None,
    }
}
