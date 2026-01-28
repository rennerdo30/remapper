//! Linux platform implementation using evdev and uinput

mod input;
mod monitor;
mod output;

pub use input::{LinuxInputBackend, LinuxInputDevice};
pub use monitor::LinuxDeviceMonitor;
pub use output::{LinuxOutputBackend, LinuxOutputDevice};
