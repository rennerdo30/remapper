//! Remapper library
//!
//! Cross-platform input remapping tool.
//!
//! ## Platform Support
//!
//! - **Linux**: Full support using evdev for input and uinput for virtual devices
//! - **Windows**: Full support using gilrs/Raw Input for input, Vigem/SendInput for output
//! - **macOS**: Partial support - keyboard/mouse remapping via CGEventPost, gamepad output pending

// Allow dead code in development - there are some unused public APIs prepared for future use
#![allow(dead_code)]

pub mod config;
pub mod core;
#[cfg(target_os = "linux")]
pub mod daemon;
pub mod devices;
pub mod mappings;
pub mod platform;

pub use config::{ConfigManager, Profile};
pub use core::RemapEngine;
#[cfg(target_os = "linux")]
pub use daemon::{DaemonConnectionState, DaemonConnector, ProfileStatus};
pub use devices::{DeviceManager, InputDevice, OutputDevice};
pub use platform::{
    DeviceType, InputBackend, OutputBackend, PlatformDeviceInfo, PlatformInputDevice,
    PlatformInputEvent, PlatformOutputDevice,
};
