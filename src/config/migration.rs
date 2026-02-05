//! Migration from v1 (Python) configuration format

use serde_json::Value;
use tracing::{debug, info};

use crate::core::error::{RemapperError, Result};
use crate::core::events::EventCode;

use super::schema::{
    Config, DeviceMatch, Mapping, OutputConfig, Profile, Settings, CONFIG_VERSION,
};

/// Migrate v1 (Python) configuration to v2 format
///
/// V1 format (Python):
/// ```json
/// {
///   "remaps": [
///     {
///       "input_device": "/dev/input/event5",
///       "output_device_name": "Remapped Device",
///       "event_map": {
///         "304": 305,
///         "305": 304
///       },
///       "grab": true
///     }
///   ]
/// }
/// ```
pub fn migrate_v1_config(v1: &Value) -> Result<Config> {
    info!("Migrating v1 configuration to v2 format");

    let mut config = Config {
        version: CONFIG_VERSION,
        settings: Settings::default(),
        profiles: Vec::new(),
    };

    // Get remaps array
    let remaps = v1
        .get("remaps")
        .and_then(|r| r.as_array())
        .ok_or_else(|| RemapperError::ConfigParseError("Missing 'remaps' array".into()))?;

    for (idx, remap) in remaps.iter().enumerate() {
        let profile = migrate_v1_remap(remap, idx)?;
        config.profiles.push(profile);
    }

    info!("Migrated {} profiles", config.profiles.len());
    Ok(config)
}

/// Migrate a single v1 remap entry to a v2 profile
fn migrate_v1_remap(remap: &Value, idx: usize) -> Result<Profile> {
    // Get input device
    let input_device = if let Some(path) = remap.get("input_device").and_then(|v| v.as_str()) {
        DeviceMatch::by_path(path)
    } else {
        return Err(RemapperError::ConfigParseError(
            "Missing input_device in remap".into(),
        ));
    };

    // Get output device name
    let output_name = remap
        .get("output_device_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Get grab setting
    let grab = remap
        .get("grab")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Convert event_map to mappings
    let mut mappings = Vec::new();

    if let Some(event_map) = remap.get("event_map").and_then(|v| v.as_object()) {
        for (from_code, to_value) in event_map {
            // Parse from code (string representation of number)
            let from_code_num: u16 = from_code.parse().map_err(|_| {
                RemapperError::ConfigParseError(format!("Invalid event code: {}", from_code))
            })?;

            // Parse to code
            let to_code_num = to_value.as_i64().ok_or_else(|| {
                RemapperError::ConfigParseError("Invalid target event code".into())
            })? as u16;

            // Convert to named codes
            let from_name = code_number_to_name(from_code_num);
            let to_name = code_number_to_name(to_code_num);

            debug!("Migrating mapping: {} ({}) -> {} ({})",
                   from_code, from_name, to_code_num, to_name);

            mappings.push(Mapping::simple(
                EventCode::key(&from_name),
                EventCode::key(&to_name),
            ));
        }
    }

    // Generate profile name
    let name = remap
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Profile {}", idx + 1));

    Ok(Profile {
        name,
        enabled: true,
        input_device,
        output_device: OutputConfig { name: output_name },
        grab,
        mappings,
    })
}

/// Convert a numeric event code to a named code
///
/// This handles common gamepad and keyboard codes
fn code_number_to_name(code: u16) -> String {
    match code {
        // Gamepad buttons (BTN_* range starts at 0x120 = 288)
        304 => "BTN_SOUTH".to_string(),
        305 => "BTN_EAST".to_string(),
        306 => "BTN_C".to_string(),
        307 => "BTN_NORTH".to_string(),
        308 => "BTN_WEST".to_string(),
        309 => "BTN_Z".to_string(),
        310 => "BTN_TL".to_string(),
        311 => "BTN_TR".to_string(),
        312 => "BTN_TL2".to_string(),
        313 => "BTN_TR2".to_string(),
        314 => "BTN_SELECT".to_string(),
        315 => "BTN_START".to_string(),
        316 => "BTN_MODE".to_string(),
        317 => "BTN_THUMBL".to_string(),
        318 => "BTN_THUMBR".to_string(),

        // Note: BTN_A=BTN_SOUTH(304), BTN_B=BTN_EAST(305), BTN_X=BTN_NORTH(307), BTN_Y=BTN_WEST(308)
        // These are aliases for the same codes, handled above

        // D-pad buttons
        544 => "BTN_DPAD_UP".to_string(),
        545 => "BTN_DPAD_DOWN".to_string(),
        546 => "BTN_DPAD_LEFT".to_string(),
        547 => "BTN_DPAD_RIGHT".to_string(),

        // Mouse buttons
        272 => "BTN_LEFT".to_string(),
        273 => "BTN_RIGHT".to_string(),
        274 => "BTN_MIDDLE".to_string(),
        275 => "BTN_SIDE".to_string(),
        276 => "BTN_EXTRA".to_string(),

        // Keyboard keys (partial list)
        1 => "KEY_ESC".to_string(),
        2 => "KEY_1".to_string(),
        3 => "KEY_2".to_string(),
        4 => "KEY_3".to_string(),
        5 => "KEY_4".to_string(),
        6 => "KEY_5".to_string(),
        7 => "KEY_6".to_string(),
        8 => "KEY_7".to_string(),
        9 => "KEY_8".to_string(),
        10 => "KEY_9".to_string(),
        11 => "KEY_0".to_string(),
        14 => "KEY_BACKSPACE".to_string(),
        15 => "KEY_TAB".to_string(),
        28 => "KEY_ENTER".to_string(),
        29 => "KEY_LEFTCTRL".to_string(),
        42 => "KEY_LEFTSHIFT".to_string(),
        54 => "KEY_RIGHTSHIFT".to_string(),
        56 => "KEY_LEFTALT".to_string(),
        57 => "KEY_SPACE".to_string(),
        97 => "KEY_RIGHTCTRL".to_string(),
        100 => "KEY_RIGHTALT".to_string(),
        125 => "KEY_LEFTMETA".to_string(),
        126 => "KEY_RIGHTMETA".to_string(),

        // Default: use numeric code
        _ => format!("KEY_{}", code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_v1_config() {
        let v1_json = r#"{
            "remaps": [
                {
                    "input_device": "/dev/input/event5",
                    "output_device_name": "Remapped Controller",
                    "event_map": {
                        "304": 305,
                        "305": 304
                    },
                    "grab": true
                }
            ]
        }"#;

        let v1: Value = serde_json::from_str(v1_json).unwrap();
        let config = migrate_v1_config(&v1).unwrap();

        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.profiles.len(), 1);

        let profile = &config.profiles[0];
        assert_eq!(profile.name, "Profile 1");
        assert!(profile.grab);
        assert_eq!(profile.mappings.len(), 2);
    }

    #[test]
    fn test_code_number_to_name() {
        assert_eq!(code_number_to_name(304), "BTN_SOUTH");
        assert_eq!(code_number_to_name(305), "BTN_EAST");
        assert_eq!(code_number_to_name(1), "KEY_ESC");
        assert_eq!(code_number_to_name(57), "KEY_SPACE");
        assert_eq!(code_number_to_name(9999), "KEY_9999");
    }
}
