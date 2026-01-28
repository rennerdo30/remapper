//! Remapper library
//!
//! Cross-platform input remapping tool.
//!
//! ## Platform Support
//!
//! - **Linux**: Full support using evdev for input and uinput for virtual devices
//! - **Windows**: Full support using gilrs/Raw Input for input, Vigem/SendInput for output
//! - **macOS**: Partial support - keyboard/mouse remapping via CGEventPost, gamepad output pending

pub mod config;
pub mod core;
pub mod devices;
pub mod mappings;
pub mod platform;

pub use config::{Config, ConfigManager, Profile};
pub use core::{RemapEngine, RemapperError};
pub use devices::{DeviceManager, InputDevice, OutputDevice};
pub use platform::{
    DeviceType, InputBackend, OutputBackend, PlatformDeviceInfo, PlatformInputDevice,
    PlatformInputEvent, PlatformOutputDevice,
};
