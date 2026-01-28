//! macOS platform implementation using gilrs, IOKit, and Core Graphics

mod input;
mod monitor;
mod output;

pub use input::MacOSInputBackend;
pub use monitor::MacOSDeviceMonitor;
pub use output::MacOSOutputBackend;
