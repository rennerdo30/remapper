//! Command-line interface

mod commands;
mod create;
mod run;
#[cfg(target_os = "linux")]
mod service;

pub use commands::{Cli, Commands, ListCommand};
#[cfg(target_os = "linux")]
pub use commands::ServiceAction;

use anyhow::Result;
use tracing::info;

use crate::config::ConfigManager;
use crate::devices::DeviceManager;

/// Run the CLI with the given arguments
pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Create { name, device } => {
            create::run_create(name, device).await?;
        }
        Commands::Edit { name } => {
            edit_profile(&name).await?;
        }
        Commands::Delete { name } => {
            delete_profile(&name).await?;
        }
        Commands::List { what } => {
            list_command(what).await?;
        }
        Commands::Run { profiles, daemon } => {
            run::run_profiles(profiles, daemon).await?;
        }
        Commands::Debug { device } => {
            debug_device(&device).await?;
        }
        Commands::Gui => {
            crate::gui::run_gui().await?;
        }
        #[cfg(target_os = "linux")]
        Commands::Service { action } => {
            service::handle_service(action).await?;
        }
    }

    Ok(())
}

/// Edit an existing profile
async fn edit_profile(name: &str) -> Result<()> {
    let config = ConfigManager::load()?;

    if config.get_profile(name).is_none() {
        anyhow::bail!("Profile not found: {}", name);
    }

    // Open config in default editor
    let config_path = config.path();
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

    info!("Opening {} with {}", config_path.display(), editor);

    let status = std::process::Command::new(&editor)
        .arg(config_path)
        .status()?;

    if status.success() {
        println!("Profile edited. Run 'remapper run \"{}\"' to apply changes.", name);
    } else {
        anyhow::bail!("Editor exited with error");
    }

    Ok(())
}

/// Delete a profile
async fn delete_profile(name: &str) -> Result<()> {
    use dialoguer::Confirm;

    let mut config = ConfigManager::load()?;

    if config.get_profile(name).is_none() {
        anyhow::bail!("Profile not found: {}", name);
    }

    let confirmed = Confirm::new()
        .with_prompt(format!("Delete profile '{}'?", name))
        .default(false)
        .interact()?;

    if confirmed {
        config.delete_profile(name)?;
        println!("Profile '{}' deleted.", name);
    } else {
        println!("Cancelled.");
    }

    Ok(())
}

/// Handle list commands
async fn list_command(what: ListCommand) -> Result<()> {
    match what {
        ListCommand::Devices => {
            list_devices().await?;
        }
        ListCommand::Profiles => {
            list_profiles().await?;
        }
    }
    Ok(())
}

/// List available input devices
async fn list_devices() -> Result<()> {
    use console::style;

    println!("{}", style("Available Input Devices:").bold());
    println!();

    let devices = DeviceManager::list_devices()?;

    if devices.is_empty() {
        println!("  No input devices found.");
        #[cfg(target_os = "linux")]
        println!("  Make sure you have permission to access /dev/input devices.");
        #[cfg(target_os = "windows")]
        println!("  Connect a gamepad to see it listed here.");
        #[cfg(target_os = "macos")]
        {
            println!("  Connect a gamepad to see it listed here.");
            println!("  Note: Keyboard/mouse detection requires IOKit permissions.");
        }
        return Ok(());
    }

    for device in devices {
        let type_badge = if device.is_gamepad {
            style("[Gamepad]").green()
        } else if device.is_keyboard {
            style("[Keyboard]").blue()
        } else if device.is_mouse {
            style("[Mouse]").yellow()
        } else {
            style("[Other]").dim()
        };

        println!(
            "  {} {} {}",
            type_badge,
            style(&device.name).bold(),
            style(format!("({})", device.path.display())).dim()
        );

        if let Some(phys) = &device.phys {
            println!("      Physical: {}", style(phys).dim());
        }
        println!(
            "      ID: {}",
            style(format!("{:04x}:{:04x}", device.vendor, device.product)).dim()
        );
        println!();
    }

    Ok(())
}

/// List configured profiles
async fn list_profiles() -> Result<()> {
    use console::style;

    let config = ConfigManager::load()?;
    let profiles = config.profiles();

    println!("{}", style("Configured Profiles:").bold());
    println!();

    if profiles.is_empty() {
        println!("  No profiles configured.");
        println!("  Use 'remapper create' to create a new profile.");
        return Ok(());
    }

    for profile in profiles {
        let status = if profile.enabled {
            style("[Enabled]").green()
        } else {
            style("[Disabled]").dim()
        };

        println!("  {} {}", status, style(&profile.name).bold());
        println!(
            "      Input: {}",
            style(profile.input_device.display()).dim()
        );
        println!(
            "      Mappings: {}",
            style(format!("{} configured", profile.mappings.len())).dim()
        );
        if profile.grab {
            println!("      Grab: {}", style("exclusive").yellow());
        }
        println!();
    }

    Ok(())
}

/// Debug a device - show raw events
async fn debug_device(device: &str) -> Result<()> {
    use console::style;

    println!("{}", style("Debug Mode - Press Ctrl+C to exit").bold());
    println!();

    // Find device - handle both paths and names
    #[cfg(target_os = "linux")]
    let path = if device.starts_with("/dev/") {
        std::path::PathBuf::from(device)
    } else {
        DeviceManager::find_by_name(device)?
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device))?
            .path
    };

    #[cfg(not(target_os = "linux"))]
    let path = {
        // On non-Linux platforms, try to find by name or use as device ID
        if device.starts_with("gamepad:") {
            // Direct device ID
            std::path::PathBuf::from(device)
        } else if device.starts_with("/dev/") {
            // Linux-style path doesn't work on other platforms
            anyhow::bail!(
                "Linux device paths like '{}' are not supported on this platform.\n\
                 Use 'remapper list devices' to see available devices.",
                device
            );
        } else {
            // Search by name
            DeviceManager::find_by_name(device)?
                .ok_or_else(|| anyhow::anyhow!(
                    "Device not found: {}\n\
                     Use 'remapper list devices' to see available devices.",
                    device
                ))?
                .path
        }
    };

    println!("Reading events from: {}", path.display());
    println!();

    let mut input = crate::devices::InputDevice::open(&path).await?;

    println!(
        "{:<12} {:<8} {:<24} {}",
        style("TYPE").bold(),
        style("CODE").bold(),
        style("NAME").bold(),
        style("VALUE").bold()
    );
    println!("{}", "-".repeat(60));

    loop {
        if let Some(event) = input.read_event().await? {
            // Skip sync events in output
            if event.is_sync() {
                continue;
            }

            let type_str = format!("{}", event.event_type);
            let code_name = get_code_name(&event);

            let value_str = if event.value == 1 {
                style("PRESS".to_string()).yellow()
            } else if event.value == 0 {
                style("RELEASE".to_string()).dim()
            } else {
                style(format!("{}", event.value)).white()
            };
            println!(
                "{:<12} {:<8} {:<24} {}",
                style(&type_str).cyan(),
                event.code,
                style(&code_name).green(),
                value_str
            );
        }
    }
}

/// Get a human-readable name for an event code
fn get_code_name(event: &crate::core::events::InputEvent) -> String {
    use crate::core::events::{abs_code_to_name, key_code_to_name, rel_code_to_name, EventType};

    match event.event_type {
        EventType::Key => key_code_to_name(event.code),
        EventType::Abs => abs_code_to_name(event.code),
        EventType::Rel => rel_code_to_name(event.code),
        _ => format!("CODE_{}", event.code),
    }
}
