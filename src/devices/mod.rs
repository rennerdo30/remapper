//! Device handling for input and output devices
//!
//! This module provides a unified interface for device handling across platforms.
//! On Linux, it uses the native evdev/uinput implementation for full functionality.
//! On Windows and macOS, it uses the platform abstraction layer.

#[cfg(target_os = "linux")]
mod input;
#[cfg(target_os = "linux")]
mod manager;
#[cfg(target_os = "linux")]
mod monitor;
#[cfg(target_os = "linux")]
mod output;

// Re-export Linux-specific types for backwards compatibility
#[cfg(target_os = "linux")]
pub use input::InputDevice;
#[cfg(target_os = "linux")]
pub use manager::{DeviceInfo, DeviceManager};
#[cfg(target_os = "linux")]
pub use monitor::DeviceMonitor;
#[cfg(target_os = "linux")]
pub use output::OutputDevice;

// For non-Linux platforms, provide wrapper types using the platform abstraction
#[cfg(not(target_os = "linux"))]
mod cross_platform {
    use std::path::PathBuf;

    use crate::core::error::Result;
    use crate::platform::{
        self, DeviceCapabilities, DeviceMonitor as PlatformDeviceMonitorTrait,
        DeviceType, InputBackend, OutputBackend, PlatformDeviceInfo, PlatformInputDevice,
    };

    // Use pollster for lightweight blocking on async code
    // This is safe to call from both sync and async contexts
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        pollster::block_on(future)
    }

    /// Information about a discovered device (cross-platform wrapper)
    #[derive(Debug, Clone)]
    pub struct DeviceInfo {
        pub path: PathBuf,
        pub name: String,
        pub phys: Option<String>,
        pub uniq: Option<String>,
        pub vendor: u16,
        pub product: u16,
        pub version: u16,
        pub is_keyboard: bool,
        pub is_mouse: bool,
        pub is_gamepad: bool,
    }

    impl From<PlatformDeviceInfo> for DeviceInfo {
        fn from(info: PlatformDeviceInfo) -> Self {
            Self {
                path: info.path.unwrap_or_else(|| PathBuf::from(&info.id)),
                name: info.name,
                phys: None,
                uniq: None,
                vendor: info.vendor_id,
                product: info.product_id,
                version: 0,
                is_keyboard: info.device_type == DeviceType::Keyboard,
                is_mouse: info.device_type == DeviceType::Mouse,
                is_gamepad: info.device_type == DeviceType::Gamepad,
            }
        }
    }

    impl DeviceInfo {
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

        pub fn id_string(&self) -> String {
            format!("{:04x}:{:04x}", self.vendor, self.product)
        }
    }

    /// Device manager for discovering and listing input devices
    pub struct DeviceManager;

    impl DeviceManager {
        pub fn list_devices() -> Result<Vec<DeviceInfo>> {
            let backend = platform::input_backend();
            let devices = block_on(InputBackend::list_devices(&backend))?;
            Ok(devices.into_iter().map(DeviceInfo::from).collect())
        }

        pub fn find_by_name(name: &str) -> Result<Option<DeviceInfo>> {
            let backend = platform::input_backend();
            let device = block_on(InputBackend::find_by_name(&backend, name))?;
            Ok(device.map(DeviceInfo::from))
        }

        pub fn find_by_id(vendor: u16, product: u16) -> Result<Option<DeviceInfo>> {
            let backend = platform::input_backend();
            let device = block_on(InputBackend::find_by_id(&backend, vendor, product))?;
            Ok(device.map(DeviceInfo::from))
        }

        pub fn list_gamepads() -> Result<Vec<DeviceInfo>> {
            Ok(Self::list_devices()?
                .into_iter()
                .filter(|d| d.is_gamepad)
                .collect())
        }

        pub fn list_keyboards() -> Result<Vec<DeviceInfo>> {
            Ok(Self::list_devices()?
                .into_iter()
                .filter(|d| d.is_keyboard)
                .collect())
        }

        pub fn list_mice() -> Result<Vec<DeviceInfo>> {
            Ok(Self::list_devices()?
                .into_iter()
                .filter(|d| d.is_mouse)
                .collect())
        }
    }

    /// Wrapper around platform input device
    pub struct InputDevice {
        inner: Box<dyn PlatformInputDevice>,
        info: DeviceInfo,
    }

    impl InputDevice {
        pub async fn open(path: &std::path::Path) -> Result<Self> {
            let backend = platform::input_backend();
            let inner: Box<dyn PlatformInputDevice> = InputBackend::open_device(&backend, &path.display().to_string()).await?;
            let platform_info = inner.info().clone();
            let info = DeviceInfo::from(platform_info);
            Ok(Self { inner, info })
        }

        pub fn name(&self) -> &str {
            &self.info.name
        }

        pub fn path(&self) -> &str {
            self.info.path.to_str().unwrap_or("")
        }

        pub fn is_grabbed(&self) -> bool {
            self.inner.is_grabbed()
        }

        pub async fn grab(&mut self) -> Result<()> {
            self.inner.grab().await
        }

        pub async fn ungrab(&mut self) -> Result<()> {
            self.inner.ungrab().await
        }

        pub async fn read_event(&mut self) -> Result<Option<crate::core::events::InputEvent>> {
            let event = self.inner.read_event().await?;
            Ok(event.map(|e| crate::core::events::InputEvent::from_platform(&e)))
        }

        pub fn capabilities(&self) -> DeviceCapabilities {
            self.inner.capabilities()
        }
    }

    /// Virtual output device wrapper
    pub struct OutputDevice {
        inner: Box<dyn crate::platform::PlatformOutputDevice>,
        name: String,
    }

    impl OutputDevice {
        pub fn create(name: &str, input: &InputDevice) -> Result<Self> {
            let backend = platform::output_backend();
            let capabilities = input.capabilities();
            let inner = OutputBackend::create_device(&backend, name, &capabilities)?;
            Ok(Self {
                inner,
                name: name.to_string(),
            })
        }

        pub fn name(&self) -> &str {
            &self.name
        }

        pub fn write_event(&self, event: &crate::core::events::InputEvent) -> Result<()> {
            let platform_event = event.to_platform();
            self.inner.write_event(&platform_event)
        }

        pub fn sync(&self) -> Result<()> {
            self.inner.sync()
        }
    }

    /// Device monitor wrapper
    pub struct DeviceMonitor;

    impl DeviceMonitor {
        pub fn new() -> Result<Self> {
            Ok(Self)
        }

        pub async fn start(
            self,
        ) -> tokio::sync::mpsc::Receiver<crate::platform::DeviceChangeEvent> {
            let monitor = platform::device_monitor().expect("Failed to create device monitor");
            PlatformDeviceMonitorTrait::start(monitor).await
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub use cross_platform::{DeviceInfo, DeviceManager, InputDevice, OutputDevice};
