//! Platform abstraction layer for cross-platform input/output device handling
//!
//! This module provides a unified interface for interacting with input devices
//! across different operating systems:
//!
//! - **Linux**: Uses evdev for input and uinput for virtual device creation
//! - **Windows**: Uses gilrs + Raw Input for input, Vigem + SendInput for output
//! - **macOS**: Uses gilrs + IOKit for input, CGEventPost for keyboard/mouse output

mod traits;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

// Re-export traits
pub use traits::{
    DeviceCapabilities, DeviceChangeEvent, DeviceMonitor, DeviceType, InputBackend,
    OutputBackend, PlatformDeviceInfo, PlatformInputDevice, PlatformInputEvent,
    PlatformOutputDevice,
};

/// Get the current platform's input backend
#[cfg(target_os = "linux")]
pub fn input_backend() -> impl InputBackend {
    linux::LinuxInputBackend::new()
}

#[cfg(target_os = "windows")]
pub fn input_backend() -> impl InputBackend {
    windows::WindowsInputBackend::new()
}

#[cfg(target_os = "macos")]
pub fn input_backend() -> impl InputBackend {
    macos::MacOSInputBackend::new()
}

/// Get the current platform's output backend
#[cfg(target_os = "linux")]
pub fn output_backend() -> impl OutputBackend {
    linux::LinuxOutputBackend::new()
}

#[cfg(target_os = "windows")]
pub fn output_backend() -> impl OutputBackend {
    windows::WindowsOutputBackend::new()
}

#[cfg(target_os = "macos")]
pub fn output_backend() -> impl OutputBackend {
    macos::MacOSOutputBackend::new()
}

/// Create a device monitor for the current platform
#[cfg(target_os = "linux")]
pub fn device_monitor() -> crate::core::error::Result<impl DeviceMonitor> {
    linux::LinuxDeviceMonitor::new()
}

#[cfg(target_os = "windows")]
pub fn device_monitor() -> crate::core::error::Result<impl DeviceMonitor> {
    windows::WindowsDeviceMonitor::new()
}

#[cfg(target_os = "macos")]
pub fn device_monitor() -> crate::core::error::Result<impl DeviceMonitor> {
    macos::MacOSDeviceMonitor::new()
}

/// Get the name of the current platform
pub fn platform_name() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "Linux"
    }
    #[cfg(target_os = "windows")]
    {
        "Windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macOS"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "Unknown"
    }
}

/// Check if the current platform is fully supported
pub fn is_fully_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        true
    }
    #[cfg(target_os = "windows")]
    {
        true // Requires Vigem for gamepad output
    }
    #[cfg(target_os = "macos")]
    {
        false // Gamepad output requires DriverKit approval
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

/// Get platform-specific notes or warnings
pub fn platform_notes() -> Option<&'static str> {
    #[cfg(target_os = "linux")]
    {
        None
    }
    #[cfg(target_os = "windows")]
    {
        Some("Virtual gamepad support requires ViGEmBus driver: https://github.com/ViGEm/ViGEmBus/releases")
    }
    #[cfg(target_os = "macos")]
    {
        Some("Virtual gamepad output is not yet supported on macOS (requires DriverKit approval from Apple). Keyboard/mouse remapping works.")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Some("This platform is not supported")
    }
}
