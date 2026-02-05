//! Configuration file management

use std::fs;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use tracing::{debug, info, warn};

use crate::core::error::{RemapperError, Result};

use super::migration::migrate_v1_config;
use super::schema::{Config, Profile, CONFIG_VERSION};

/// Configuration file manager
#[derive(Debug, Clone)]
pub struct ConfigManager {
    /// Path to config file
    path: PathBuf,
    /// Current configuration
    config: Config,
}

impl ConfigManager {
    /// Get the default config directory
    pub fn default_config_dir() -> Option<PathBuf> {
        ProjectDirs::from("", "", "remapper").map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Get the default config file path
    pub fn default_config_path() -> Option<PathBuf> {
        Self::default_config_dir().map(|dir| dir.join("config.json"))
    }

    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        let path = Self::default_config_path()
            .ok_or_else(|| RemapperError::ConfigError("Could not determine config path".into()))?;
        Self::load_from(&path)
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            info!("Config file not found, creating default: {}", path.display());

            // Create directory if needed
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Create default config
            let config = Config::default();
            let manager = Self {
                path: path.to_path_buf(),
                config,
            };
            manager.save()?;
            return Ok(manager);
        }

        debug!("Loading config from: {}", path.display());
        let content = fs::read_to_string(path)?;

        // Try to parse and check version
        let config: Config = match serde_json::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                // Try to detect v1 format (no version field)
                if let Ok(v1_value) = serde_json::from_str::<serde_json::Value>(&content) {
                    if v1_value.get("version").is_none() {
                        info!("Detected v1 config format, migrating...");
                        let migrated = migrate_v1_config(&v1_value)?;

                        // Backup old config
                        let backup_path = path.with_extension("v1.bak.json");
                        fs::write(&backup_path, &content)?;
                        info!("Backed up v1 config to: {}", backup_path.display());

                        migrated
                    } else {
                        return Err(RemapperError::ConfigParseError(e.to_string()));
                    }
                } else {
                    return Err(RemapperError::ConfigParseError(e.to_string()));
                }
            }
        };

        // Check version
        if config.version > CONFIG_VERSION {
            warn!(
                "Config version {} is newer than supported version {}",
                config.version, CONFIG_VERSION
            );
        }

        let manager = Self {
            path: path.to_path_buf(),
            config,
        };

        // Save if we migrated
        if manager.config.version < CONFIG_VERSION {
            manager.save()?;
        }

        Ok(manager)
    }

    /// Get the config file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the current configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        debug!("Saving config to: {}", self.path.display());

        // Ensure directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.path, content)?;

        info!("Config saved to: {}", self.path.display());
        Ok(())
    }

    /// Create a backup of the current config
    pub fn backup(&self) -> Result<PathBuf> {
        let timestamp = chrono_lite_timestamp();
        let backup_name = format!(
            "config.backup.{}.json",
            timestamp
        );
        let backup_path = self.path.parent()
            .map(|p| p.join(&backup_name))
            .unwrap_or_else(|| PathBuf::from(&backup_name));

        fs::copy(&self.path, &backup_path)?;
        info!("Created backup: {}", backup_path.display());
        Ok(backup_path)
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> Result<()> {
        let new_manager = Self::load_from(&self.path)?;
        self.config = new_manager.config;
        info!("Config reloaded");
        Ok(())
    }

    /// Get all profiles
    pub fn profiles(&self) -> &[Profile] {
        &self.config.profiles
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.config.find_profile(name)
    }

    /// Add a new profile
    pub fn add_profile(&mut self, profile: Profile) -> Result<()> {
        self.config.add_profile(profile)?;
        self.save()
    }

    /// Update an existing profile
    pub fn update_profile(&mut self, name: &str, profile: Profile) -> Result<()> {
        // Remove old profile
        self.config.remove_profile(name)?;
        // Add new profile
        self.config.profiles.push(profile);
        self.save()
    }

    /// Delete a profile
    pub fn delete_profile(&mut self, name: &str) -> Result<()> {
        self.config.remove_profile(name)?;
        self.save()
    }

    /// Get enabled profiles
    pub fn enabled_profiles(&self) -> Vec<&Profile> {
        self.config.enabled_profiles()
    }

    /// Set profile enabled state
    pub fn set_profile_enabled(&mut self, name: &str, enabled: bool) -> Result<()> {
        if let Some(profile) = self.config.find_profile_mut(name) {
            profile.enabled = enabled;
            self.save()?;
            Ok(())
        } else {
            Err(RemapperError::ProfileNotFound(name.to_string()))
        }
    }
}

/// Generate a simple timestamp without external dependencies
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_default_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let manager = ConfigManager::load_from(&path).unwrap();
        assert!(path.exists());
        assert_eq!(manager.config.version, CONFIG_VERSION);
        assert!(manager.config.profiles.is_empty());
    }

    #[test]
    fn test_add_profile() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let mut manager = ConfigManager::load_from(&path).unwrap();

        let profile = Profile::new(
            "Test Profile",
            super::super::schema::DeviceMatch::by_name("Test Device"),
        );

        manager.add_profile(profile).unwrap();

        assert_eq!(manager.profiles().len(), 1);
        assert!(manager.get_profile("Test Profile").is_some());
    }
}
