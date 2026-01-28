//! Configuration schema definitions

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::core::events::EventCode;
use crate::core::error::{RemapperError, Result};
use crate::devices::DeviceManager;

/// Current configuration version
pub const CONFIG_VERSION: u32 = 2;

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Configuration version
    #[serde(default = "default_version")]
    pub version: u32,
    /// Global settings
    #[serde(default)]
    pub settings: Settings,
    /// Configured profiles
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            settings: Settings::default(),
            profiles: Vec::new(),
        }
    }
}

impl Config {
    /// Create a new empty configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Find a profile by name
    pub fn find_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    /// Find a profile by name (mutable)
    pub fn find_profile_mut(&mut self, name: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.name == name)
    }

    /// Get enabled profiles
    pub fn enabled_profiles(&self) -> Vec<&Profile> {
        self.profiles.iter().filter(|p| p.enabled).collect()
    }

    /// Add a new profile
    pub fn add_profile(&mut self, profile: Profile) -> Result<()> {
        if self.find_profile(&profile.name).is_some() {
            return Err(RemapperError::ProfileExists(profile.name.clone()));
        }
        self.profiles.push(profile);
        Ok(())
    }

    /// Remove a profile by name
    pub fn remove_profile(&mut self, name: &str) -> Result<Profile> {
        let idx = self
            .profiles
            .iter()
            .position(|p| p.name == name)
            .ok_or_else(|| RemapperError::ProfileNotFound(name.to_string()))?;
        Ok(self.profiles.remove(idx))
    }
}

/// Global settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Auto-start enabled profiles on launch
    #[serde(default)]
    pub auto_start: bool,
    /// Enable hotplug monitoring
    #[serde(default = "default_true")]
    pub hotplug_enabled: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            auto_start: false,
            hotplug_enabled: true,
        }
    }
}

/// Profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name (unique identifier)
    pub name: String,
    /// Whether this profile is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Input device to remap
    pub input_device: DeviceMatch,
    /// Output device configuration
    #[serde(default)]
    pub output_device: OutputConfig,
    /// Whether to grab exclusive access to input device
    #[serde(default)]
    pub grab: bool,
    /// Event mappings
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

impl Profile {
    /// Create a new profile
    pub fn new(name: impl Into<String>, input_device: DeviceMatch) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            input_device,
            output_device: OutputConfig::default(),
            grab: false,
            mappings: Vec::new(),
        }
    }

    /// Add a mapping to this profile
    pub fn add_mapping(&mut self, mapping: Mapping) {
        self.mappings.push(mapping);
    }
}

/// Device matching criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeviceMatch {
    /// Match by device path
    Path {
        path: String,
    },
    /// Match by name (partial match)
    Name {
        name: String,
    },
    /// Match by vendor and product ID
    Id {
        vendor: u16,
        product: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

impl DeviceMatch {
    /// Create a match by path
    pub fn by_path(path: impl Into<String>) -> Self {
        DeviceMatch::Path { path: path.into() }
    }

    /// Create a match by name
    pub fn by_name(name: impl Into<String>) -> Self {
        DeviceMatch::Name { name: name.into() }
    }

    /// Create a match by vendor/product ID
    pub fn by_id(vendor: u16, product: u16) -> Self {
        DeviceMatch::Id {
            vendor,
            product,
            name: None,
        }
    }

    /// Resolve this match to a device path
    pub fn resolve(&self) -> Result<PathBuf> {
        match self {
            DeviceMatch::Path { path } => {
                let path = PathBuf::from(path);
                if path.exists() {
                    Ok(path)
                } else {
                    Err(RemapperError::DeviceNotFound(path.display().to_string()))
                }
            }
            DeviceMatch::Name { name } => {
                DeviceManager::find_by_name(name)?
                    .map(|d| d.path)
                    .ok_or_else(|| RemapperError::DeviceNotFound(name.clone()))
            }
            DeviceMatch::Id { vendor, product, .. } => {
                DeviceManager::find_by_id(*vendor, *product)?
                    .map(|d| d.path)
                    .ok_or_else(|| {
                        RemapperError::DeviceNotFound(format!("{:04x}:{:04x}", vendor, product))
                    })
            }
        }
    }

    /// Get a display string for this match
    pub fn display(&self) -> String {
        match self {
            DeviceMatch::Path { path } => path.clone(),
            DeviceMatch::Name { name } => name.clone(),
            DeviceMatch::Id { vendor, product, name } => {
                if let Some(n) = name {
                    format!("{} ({:04x}:{:04x})", n, vendor, product)
                } else {
                    format!("{:04x}:{:04x}", vendor, product)
                }
            }
        }
    }
}

/// Output device configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Name for the virtual device
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl OutputConfig {
    /// Create output config with a custom name
    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
        }
    }
}

/// Event mapping types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Mapping {
    /// Simple 1:1 mapping
    Simple {
        from: EventCode,
        to: EventCode,
    },
    /// Macro - trigger a sequence of events
    Macro {
        trigger: EventCode,
        sequence: Vec<MacroStep>,
    },
    /// Conditional - different output based on tap/hold
    Conditional {
        trigger: EventCode,
        #[serde(skip_serializing_if = "Option::is_none")]
        tap: Option<EventCode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        hold: Option<EventCode>,
        #[serde(default = "default_threshold")]
        threshold_ms: u32,
    },
    /// Combo - trigger when multiple keys pressed together
    Combo {
        keys: Vec<EventCode>,
        output: EventCode,
        #[serde(default)]
        order_sensitive: bool,
    },
}

fn default_threshold() -> u32 {
    300
}

impl Mapping {
    /// Create a simple mapping
    pub fn simple(from: EventCode, to: EventCode) -> Self {
        Mapping::Simple { from, to }
    }

    /// Create a macro mapping
    pub fn macro_seq(trigger: EventCode, sequence: Vec<MacroStep>) -> Self {
        Mapping::Macro { trigger, sequence }
    }

    /// Create a conditional (tap/hold) mapping
    pub fn conditional(
        trigger: EventCode,
        tap: Option<EventCode>,
        hold: Option<EventCode>,
        threshold_ms: u32,
    ) -> Self {
        Mapping::Conditional {
            trigger,
            tap,
            hold,
            threshold_ms,
        }
    }

    /// Create a combo mapping
    pub fn combo(keys: Vec<EventCode>, output: EventCode) -> Self {
        Mapping::Combo {
            keys,
            output,
            order_sensitive: false,
        }
    }

    /// Get the trigger event code for this mapping
    pub fn trigger(&self) -> Option<&EventCode> {
        match self {
            Mapping::Simple { from, .. } => Some(from),
            Mapping::Macro { trigger, .. } => Some(trigger),
            Mapping::Conditional { trigger, .. } => Some(trigger),
            Mapping::Combo { keys, .. } => keys.first(),
        }
    }
}

/// Step in a macro sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MacroStep {
    /// Key event (press or release)
    Key {
        code: String,
        value: i32,
    },
    /// Delay in milliseconds
    Delay {
        delay_ms: u32,
    },
}

impl MacroStep {
    /// Create a key press step
    pub fn press(code: impl Into<String>) -> Self {
        MacroStep::Key {
            code: code.into(),
            value: 1,
        }
    }

    /// Create a key release step
    pub fn release(code: impl Into<String>) -> Self {
        MacroStep::Key {
            code: code.into(),
            value: 0,
        }
    }

    /// Create a delay step
    pub fn delay(ms: u32) -> Self {
        MacroStep::Delay { delay_ms: ms }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = Config {
            version: 2,
            settings: Settings::default(),
            profiles: vec![Profile {
                name: "Test".to_string(),
                enabled: true,
                input_device: DeviceMatch::by_name("Test Device"),
                output_device: OutputConfig::default(),
                grab: false,
                mappings: vec![Mapping::simple(
                    EventCode::key("BTN_A"),
                    EventCode::key("BTN_B"),
                )],
            }],
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.profiles.len(), 1);
        assert_eq!(parsed.profiles[0].name, "Test");
    }

    #[test]
    fn test_device_match_display() {
        assert_eq!(
            DeviceMatch::by_path("/dev/input/event5").display(),
            "/dev/input/event5"
        );
        assert_eq!(
            DeviceMatch::by_name("Xbox Controller").display(),
            "Xbox Controller"
        );
        assert_eq!(
            DeviceMatch::by_id(0x045e, 0x028e).display(),
            "045e:028e"
        );
    }
}
