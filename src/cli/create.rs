//! Interactive profile creation wizard

use anyhow::Result;
use console::style;
use dialoguer::{Confirm, Input, Select};

use crate::config::{ConfigManager, DeviceMatch, Mapping, OutputConfig, Profile};
use crate::core::events::EventCode;
use crate::devices::DeviceManager;

/// Run the interactive create wizard
pub async fn run_create(name: Option<String>, device: Option<String>) -> Result<()> {
    println!();
    println!("{}", style("Create New Profile").bold().underlined());
    println!();

    // Get profile name
    let profile_name = if let Some(name) = name {
        name
    } else {
        Input::new()
            .with_prompt("Profile name")
            .interact_text()?
    };

    // Check if profile already exists
    let config = ConfigManager::load()?;
    if config.get_profile(&profile_name).is_some() {
        anyhow::bail!("Profile '{}' already exists", profile_name);
    }
    drop(config);

    // Select input device
    let input_device = if let Some(device) = device {
        if device.starts_with("/dev/") {
            DeviceMatch::by_path(device)
        } else {
            DeviceMatch::by_name(device)
        }
    } else {
        select_input_device().await?
    };

    println!();
    println!(
        "{}",
        style(format!("Selected device: {}", input_device.display())).dim()
    );

    // Output device name
    let output_name: String = Input::new()
        .with_prompt("Virtual device name")
        .default(format!("Remapped {}", profile_name))
        .interact_text()?;

    // Grab device?
    let grab = Confirm::new()
        .with_prompt("Grab exclusive access to input device?")
        .default(false)
        .interact()?;

    println!();
    println!("{}", style("Add Mappings").bold());
    println!(
        "{}",
        style("(You can add more mappings later by editing the config)").dim()
    );
    println!();

    // Add mappings
    let mut mappings = Vec::new();
    loop {
        if !Confirm::new()
            .with_prompt("Add a mapping?")
            .default(mappings.is_empty())
            .interact()?
        {
            break;
        }

        if let Ok(mapping) = create_mapping().await {
            println!(
                "  {}",
                style(format!("Added: {:?}", mapping)).green()
            );
            mappings.push(mapping);
        }
    }

    // Create profile
    let profile = Profile {
        name: profile_name.clone(),
        enabled: true,
        input_device,
        output_device: OutputConfig {
            name: Some(output_name),
        },
        grab,
        mappings,
    };

    // Save profile
    let mut config = ConfigManager::load()?;
    config.add_profile(profile)?;

    println!();
    println!(
        "{}",
        style(format!("Profile '{}' created successfully!", profile_name)).green()
    );
    println!();
    println!("To run this profile:");
    println!(
        "  {}",
        style(format!("remapper run \"{}\"", profile_name)).cyan()
    );
    println!();

    Ok(())
}

/// Interactive device selection
async fn select_input_device() -> Result<DeviceMatch> {
    let devices = DeviceManager::list_devices()?;

    if devices.is_empty() {
        anyhow::bail!("No input devices found. Check permissions.");
    }

    let items: Vec<String> = devices
        .iter()
        .map(|d| {
            let type_str = if d.is_gamepad {
                "[Gamepad]"
            } else if d.is_keyboard {
                "[Keyboard]"
            } else if d.is_mouse {
                "[Mouse]"
            } else {
                "[Other]"
            };
            format!("{} {} ({})", type_str, d.name, d.path.display())
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Select input device")
        .items(&items)
        .default(0)
        .interact()?;

    let device = &devices[selection];

    // Ask how to match the device
    let match_options = vec![
        format!("By name: {}", device.name),
        format!("By ID: {:04x}:{:04x}", device.vendor, device.product),
        format!("By path: {}", device.path.display()),
    ];

    let match_type = Select::new()
        .with_prompt("How should this device be matched?")
        .items(&match_options)
        .default(0)
        .interact()?;

    Ok(match match_type {
        0 => DeviceMatch::by_name(&device.name),
        1 => DeviceMatch::by_id(device.vendor, device.product),
        2 => DeviceMatch::by_path(device.path.to_string_lossy()),
        _ => unreachable!(),
    })
}

/// Create a single mapping interactively
async fn create_mapping() -> Result<Mapping> {
    let mapping_types = vec!["Simple (1:1 remap)", "Macro (key sequence)", "Tap/Hold"];

    let mapping_type = Select::new()
        .with_prompt("Mapping type")
        .items(&mapping_types)
        .default(0)
        .interact()?;

    match mapping_type {
        0 => create_simple_mapping().await,
        1 => create_macro_mapping().await,
        2 => create_conditional_mapping().await,
        _ => unreachable!(),
    }
}

/// Create a simple 1:1 mapping
async fn create_simple_mapping() -> Result<Mapping> {
    println!();
    println!(
        "{}",
        style("Enter key codes (e.g., BTN_A, KEY_ESC, BTN_START)").dim()
    );

    let from: String = Input::new()
        .with_prompt("From (source key)")
        .interact_text()?;

    let to: String = Input::new()
        .with_prompt("To (target key)")
        .interact_text()?;

    Ok(Mapping::simple(EventCode::key(from), EventCode::key(to)))
}

/// Create a macro mapping
async fn create_macro_mapping() -> Result<Mapping> {
    use crate::config::MacroStep;

    println!();
    println!(
        "{}",
        style("Create a macro sequence").dim()
    );

    let trigger: String = Input::new()
        .with_prompt("Trigger key")
        .interact_text()?;

    let mut sequence = Vec::new();
    println!();
    println!(
        "{}",
        style("Add steps to the macro (press, release, delay)").dim()
    );

    loop {
        let step_types = vec!["Key press", "Key release", "Delay", "Done"];
        let step_type = Select::new()
            .with_prompt("Step type")
            .items(&step_types)
            .default(0)
            .interact()?;

        match step_type {
            0 => {
                let key: String = Input::new()
                    .with_prompt("Key to press")
                    .interact_text()?;
                sequence.push(MacroStep::press(key));
            }
            1 => {
                let key: String = Input::new()
                    .with_prompt("Key to release")
                    .interact_text()?;
                sequence.push(MacroStep::release(key));
            }
            2 => {
                let ms: u32 = Input::new()
                    .with_prompt("Delay (milliseconds)")
                    .default(50)
                    .interact()?;
                sequence.push(MacroStep::delay(ms));
            }
            3 => break,
            _ => unreachable!(),
        }
    }

    Ok(Mapping::macro_seq(EventCode::key(trigger), sequence))
}

/// Create a conditional (tap/hold) mapping
async fn create_conditional_mapping() -> Result<Mapping> {
    println!();
    println!(
        "{}",
        style("Create tap/hold mapping").dim()
    );

    let trigger: String = Input::new()
        .with_prompt("Trigger key")
        .interact_text()?;

    let tap: String = Input::new()
        .with_prompt("Tap action (short press)")
        .allow_empty(true)
        .interact_text()?;

    let hold: String = Input::new()
        .with_prompt("Hold action (long press)")
        .allow_empty(true)
        .interact_text()?;

    let threshold: u32 = Input::new()
        .with_prompt("Hold threshold (milliseconds)")
        .default(300)
        .interact()?;

    let tap_code = if tap.is_empty() {
        None
    } else {
        Some(EventCode::key(tap))
    };

    let hold_code = if hold.is_empty() {
        None
    } else {
        Some(EventCode::key(hold))
    };

    Ok(Mapping::conditional(
        EventCode::key(trigger),
        tap_code,
        hold_code,
        threshold,
    ))
}
