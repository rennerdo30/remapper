//! Windows platform implementation using gilrs, Raw Input, Vigem, and SendInput

mod input;
mod monitor;
mod output;

pub use input::{WindowsInputBackend, WindowsInputDevice};
pub use monitor::WindowsDeviceMonitor;
pub use output::{WindowsOutputBackend, WindowsOutputDevice};
