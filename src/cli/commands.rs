//! CLI command definitions using clap

use clap::{Parser, Subcommand};

/// Cross-platform input remapping tool
#[derive(Parser)]
#[command(name = "remapper")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Create a new profile (interactive)
    Create {
        /// Profile name
        #[arg(short, long)]
        name: Option<String>,

        /// Input device path or name
        #[arg(short, long)]
        device: Option<String>,
    },

    /// Edit an existing profile
    Edit {
        /// Profile name to edit
        name: String,
    },

    /// Delete a profile
    Delete {
        /// Profile name to delete
        name: String,
    },

    /// List devices or profiles
    List {
        #[command(subcommand)]
        what: ListCommand,
    },

    /// Run profiles
    Run {
        /// Specific profiles to run (runs all enabled if none specified)
        profiles: Vec<String>,

        /// Run as background daemon
        #[arg(short, long)]
        daemon: bool,
    },

    /// Debug device - show raw events
    Debug {
        /// Device path or name
        device: String,
    },

    /// Launch graphical interface
    Gui,

    /// Manage systemd service (Linux only)
    #[cfg(target_os = "linux")]
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

/// List subcommands
#[derive(Subcommand)]
pub enum ListCommand {
    /// List available input devices
    Devices,

    /// List configured profiles
    Profiles,
}

/// Service management actions (Linux only)
#[cfg(target_os = "linux")]
#[derive(Subcommand)]
pub enum ServiceAction {
    /// Install systemd user service
    Install,

    /// Uninstall systemd user service
    Uninstall,

    /// Start the service
    Start,

    /// Stop the service
    Stop,

    /// Restart the service
    Restart,

    /// Show service status
    Status,

    /// Show service logs
    Logs {
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: usize,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },

    /// Enable service to start on login
    Enable,

    /// Disable service from starting on login
    Disable,
}
