//! Remapper - Cross-platform input remapping tool
//!
//! A tool for remapping input device events to different outputs via a virtual input device.
//!
//! ## Platform Support
//!
//! - **Linux**: Full support using evdev for input and uinput for virtual devices
//! - **Windows**: Gamepad support using gilrs, keyboard/mouse via SendInput, virtual gamepad via ViGEm
//! - **macOS**: Gamepad support using gilrs, keyboard/mouse output via CGEventPost

// Allow dead code in development - there are some unused public APIs prepared for future use
#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod cli;
mod config;
mod core;
mod daemon;
mod devices;
mod gui;
mod mappings;
mod platform;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    info!("Remapper v{}", env!("CARGO_PKG_VERSION"));

    // Show platform-specific warnings
    show_platform_info();

    // Dispatch to appropriate command handler
    cli::run(cli).await
}

/// Show platform-specific information and warnings
fn show_platform_info() {
    let platform = platform::platform_name();
    info!("Running on {}", platform);

    if !platform::is_fully_supported() {
        if let Some(notes) = platform::platform_notes() {
            warn!("Platform limitation: {}", notes);
        }
    }

    // Platform-specific additional warnings
    #[cfg(target_os = "windows")]
    {
        // Check if ViGEmBus is likely available
        info!("Virtual gamepad support requires ViGEmBus driver");
    }

    #[cfg(target_os = "macos")]
    {
        info!("Note: macOS requires 'Input Monitoring' permission in System Preferences > Privacy & Security");
    }
}
