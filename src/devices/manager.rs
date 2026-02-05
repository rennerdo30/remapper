//! Device discovery and management

use std::fs;
use std::path::{Path, PathBuf};
use evdev::Device;
use tracing::{debug, trace};

use crate::core::error::{RemapperError, Result};

/// Information about a discovered device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Device path (e.g., /dev/input/event5)
    pub path: PathBuf,
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
    /// Whether this is a keyboard
    pub is_keyboard: bool,
    /// Whether this is a mouse
    pub is_mouse: bool,
    /// Whether this is a gamepad/joystick
    pub is_gamepad: bool,
}

impl DeviceInfo {
    /// Create device info from an evdev device
    fn from_device(path: PathBuf, device: &Device) -> Self {
        let id = device.input_id();

        // Detect device type based on capabilities
        let has_keys = device.supported_keys().is_some();
        let has_abs = device.supported_absolute_axes().is_some();
        let has_rel = device.supported_relative_axes().is_some();

        // Check for gamepad-specific buttons
        let is_gamepad = device
            .supported_keys()
            .map(|keys| has_abs && keys.contains(evdev::Key::BTN_SOUTH))
            .unwrap_or(false);

        // Check for keyboard keys
        let is_keyboard = device
            .supported_keys()
            .map(|keys| {
                keys.contains(evdev::Key::KEY_A)
                    && keys.contains(evdev::Key::KEY_Z)
                    && keys.contains(evdev::Key::KEY_ENTER)
            })
            .unwrap_or(false);

        // Check for mouse
        let is_mouse = has_rel && device
            .supported_keys()
            .map(|keys| keys.contains(evdev::Key::BTN_LEFT))
            .unwrap_or(false);

        Self {
            path,
            name: device.name().unwrap_or("Unknown").to_string(),
            phys: device.physical_path().map(|s| s.to_string()),
            uniq: device.unique_name().map(|s| s.to_string()),
            vendor: id.vendor(),
            product: id.product(),
            version: id.version(),
            is_keyboard,
            is_mouse,
            is_gamepad,
        }
    }

    /// Get a display string for the device
    pub fn display_name(&self) -> String {
        let type_str = if self.is_gamepad {
            "[Gamepad]"
        } else if self.is_keyboard {
            "[Keyboard]"
        } else if self.is_mouse {
            "[Mouse]"
        } else {
            "[Other]"
        };
        format!("{} {} - {}", type_str, self.name, self.path.display())
    }

    /// Get vendor:product ID string
    pub fn id_string(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor, self.product)
    }
}

/// Device manager for discovering and listing input devices
pub struct DeviceManager;

impl DeviceManager {
    /// List all input devices
    pub fn list_devices() -> Result<Vec<DeviceInfo>> {
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
                    let info = DeviceInfo::from_device(path, &device);
                    debug!("Found device: {} ({})", info.name, info.path.display());
                    devices.push(info);
                }
                Err(e) => {
                    trace!("Could not open {}: {}", path.display(), e);
                }
            }
        }

        // Sort by path for consistent ordering
        devices.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(devices)
    }

    /// Find a device by name (partial match)
    pub fn find_by_name(name: &str) -> Result<Option<DeviceInfo>> {
        let devices = Self::list_devices()?;
        let name_lower = name.to_lowercase();

        Ok(devices
            .into_iter()
            .find(|d| d.name.to_lowercase().contains(&name_lower)))
    }

    /// Find a device by exact name
    pub fn find_by_exact_name(name: &str) -> Result<Option<DeviceInfo>> {
        let devices = Self::list_devices()?;
        Ok(devices.into_iter().find(|d| d.name == name))
    }

    /// Find a device by vendor and product ID
    pub fn find_by_id(vendor: u16, product: u16) -> Result<Option<DeviceInfo>> {
        let devices = Self::list_devices()?;
        Ok(devices
            .into_iter()
            .find(|d| d.vendor == vendor && d.product == product))
    }

    /// Find a device by path
    pub fn find_by_path(path: &Path) -> Result<Option<DeviceInfo>> {
        if !path.exists() {
            return Ok(None);
        }

        match Device::open(path) {
            Ok(device) => Ok(Some(DeviceInfo::from_device(path.to_path_buf(), &device))),
            Err(_) => Ok(None),
        }
    }

    /// List only gamepad devices
    pub fn list_gamepads() -> Result<Vec<DeviceInfo>> {
        Ok(Self::list_devices()?
            .into_iter()
            .filter(|d| d.is_gamepad)
            .collect())
    }

    /// List only keyboard devices
    pub fn list_keyboards() -> Result<Vec<DeviceInfo>> {
        Ok(Self::list_devices()?
            .into_iter()
            .filter(|d| d.is_keyboard)
            .collect())
    }

    /// List only mouse devices
    pub fn list_mice() -> Result<Vec<DeviceInfo>> {
        Ok(Self::list_devices()?
            .into_iter()
            .filter(|d| d.is_mouse)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_info_display() {
        let info = DeviceInfo {
            path: PathBuf::from("/dev/input/event0"),
            name: "Test Device".to_string(),
            phys: None,
            uniq: None,
            vendor: 0x1234,
            product: 0x5678,
            version: 1,
            is_keyboard: false,
            is_mouse: false,
            is_gamepad: true,
        };

        assert!(info.display_name().contains("[Gamepad]"));
        assert!(info.display_name().contains("Test Device"));
        assert_eq!(info.id_string(), "1234:5678");
    }
}
