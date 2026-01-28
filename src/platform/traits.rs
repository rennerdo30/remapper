//! Platform abstraction traits for input/output devices
//!
//! These traits define the interface that each platform implementation must provide.

use async_trait::async_trait;
use std::path::PathBuf;

use crate::core::error::Result;

/// Device type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceType {
    /// Keyboard device
    Keyboard,
    /// Mouse/pointing device
    Mouse,
    /// Gamepad/joystick device
    Gamepad,
    /// Unknown or other device type
    Other,
}

/// Platform-agnostic device information
#[derive(Debug, Clone)]
pub struct PlatformDeviceInfo {
    /// Unique identifier for the device (path on Linux, instance ID on Windows)
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device type
    pub device_type: DeviceType,
    /// Platform-specific path (if applicable)
    pub path: Option<PathBuf>,
    /// Whether this device supports being grabbed (exclusive access)
    pub supports_grab: bool,
}

impl PlatformDeviceInfo {
    /// Get a display string for the device
    pub fn display_name(&self) -> String {
        let type_str = match self.device_type {
            DeviceType::Keyboard => "[Keyboard]",
            DeviceType::Mouse => "[Mouse]",
            DeviceType::Gamepad => "[Gamepad]",
            DeviceType::Other => "[Other]",
        };
        format!("{} {} - {}", type_str, self.name, self.id)
    }

    /// Get vendor:product ID string
    pub fn id_string(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor_id, self.product_id)
    }
}

/// Device capabilities for creating virtual output devices
#[derive(Debug, Clone, Default)]
pub struct DeviceCapabilities {
    /// Supported key/button codes
    pub keys: Vec<u16>,
    /// Supported absolute axis codes with their info (code, min, max, fuzz, flat, resolution)
    pub abs_axes: Vec<AbsAxisInfo>,
    /// Supported relative axis codes
    pub rel_axes: Vec<u16>,
}

/// Absolute axis information
#[derive(Debug, Clone)]
pub struct AbsAxisInfo {
    /// Axis code
    pub code: u16,
    /// Current value
    pub value: i32,
    /// Minimum value
    pub minimum: i32,
    /// Maximum value
    pub maximum: i32,
    /// Fuzz value (noise threshold)
    pub fuzz: i32,
    /// Flat value (dead zone)
    pub flat: i32,
    /// Resolution (units per mm or radian)
    pub resolution: i32,
}

/// Platform-agnostic input event
#[derive(Debug, Clone)]
pub struct PlatformInputEvent {
    /// Event type (0=sync, 1=key, 2=relative, 3=absolute, etc.)
    pub event_type: u16,
    /// Event code
    pub code: u16,
    /// Event value
    pub value: i32,
    /// Timestamp in microseconds
    pub timestamp_us: u64,
}

impl PlatformInputEvent {
    /// Create a new platform input event
    pub fn new(event_type: u16, code: u16, value: i32) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        Self {
            event_type,
            code,
            value,
            timestamp_us: timestamp,
        }
    }

    /// Check if this is a sync event
    pub fn is_sync(&self) -> bool {
        self.event_type == 0
    }

    /// Check if this is a key event
    pub fn is_key(&self) -> bool {
        self.event_type == 1
    }

    /// Check if this is a key press
    pub fn is_key_press(&self) -> bool {
        self.is_key() && self.value == 1
    }

    /// Check if this is a key release
    pub fn is_key_release(&self) -> bool {
        self.is_key() && self.value == 0
    }

    /// Create a sync event
    pub fn sync() -> Self {
        Self::new(0, 0, 0)
    }

    /// Create a key press event
    pub fn key_press(code: u16) -> Self {
        Self::new(1, code, 1)
    }

    /// Create a key release event
    pub fn key_release(code: u16) -> Self {
        Self::new(1, code, 0)
    }
}

/// Device change events for hotplug monitoring
#[derive(Debug, Clone)]
pub enum DeviceChangeEvent {
    /// A new device was connected
    Added(PlatformDeviceInfo),
    /// A device was disconnected
    Removed(String), // Device ID
}

/// Trait for platform-specific input device implementations
#[async_trait]
pub trait PlatformInputDevice: Send + Sync {
    /// Read an input event from the device (async, may block)
    async fn read_event(&mut self) -> Result<Option<PlatformInputEvent>>;

    /// Grab exclusive access to the device (prevents other apps from receiving events)
    async fn grab(&mut self) -> Result<()>;

    /// Release exclusive access to the device
    async fn ungrab(&mut self) -> Result<()>;

    /// Check if the device is currently grabbed
    fn is_grabbed(&self) -> bool;

    /// Get device information
    fn info(&self) -> &PlatformDeviceInfo;

    /// Get device capabilities
    fn capabilities(&self) -> DeviceCapabilities;
}

/// Trait for platform-specific output device implementations
pub trait PlatformOutputDevice: Send + Sync {
    /// Write an event to the virtual device
    fn write_event(&self, event: &PlatformInputEvent) -> Result<()>;

    /// Write multiple events to the virtual device
    fn write_events(&self, events: &[PlatformInputEvent]) -> Result<()>;

    /// Write a sync event
    fn sync(&self) -> Result<()>;

    /// Get the device name
    fn name(&self) -> &str;
}

/// Trait for platform-specific input backend (device enumeration and opening)
#[async_trait]
pub trait InputBackend: Send + Sync {
    /// List all available input devices
    async fn list_devices(&self) -> Result<Vec<PlatformDeviceInfo>>;

    /// Open an input device by its ID
    async fn open_device(&self, device_id: &str) -> Result<Box<dyn PlatformInputDevice>>;

    /// Find a device by name (partial match)
    async fn find_by_name(&self, name: &str) -> Result<Option<PlatformDeviceInfo>>;

    /// Find a device by vendor/product ID
    async fn find_by_id(&self, vendor: u16, product: u16) -> Result<Option<PlatformDeviceInfo>>;
}

/// Trait for platform-specific output backend (virtual device creation)
pub trait OutputBackend: Send + Sync {
    /// Create a virtual output device with capabilities copied from an input device
    fn create_device(
        &self,
        name: &str,
        capabilities: &DeviceCapabilities,
    ) -> Result<Box<dyn PlatformOutputDevice>>;

    /// Check if the platform supports creating virtual devices of a given type
    fn supports_device_type(&self, device_type: DeviceType) -> bool;

    /// Check if the platform's output backend is available (e.g., Vigem installed on Windows)
    fn is_available(&self) -> bool;

    /// Get a message describing what's needed to enable the output backend
    fn availability_message(&self) -> Option<String>;
}

/// Trait for device hotplug monitoring
#[async_trait]
pub trait DeviceMonitor: Send + Sync {
    /// Start monitoring for device changes, returns a receiver for events
    async fn start(self) -> tokio::sync::mpsc::Receiver<DeviceChangeEvent>;
}
