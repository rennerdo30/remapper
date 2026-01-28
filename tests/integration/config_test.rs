//! Integration tests for configuration handling

use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

/// Get the path to test fixtures
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn test_load_v2_config() {
    let content = fs::read_to_string(fixture_path("test_config.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(config["version"], 2);
    assert_eq!(config["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(config["profiles"][0]["name"], "Test Profile");
}

#[test]
fn test_v1_config_format() {
    let content = fs::read_to_string(fixture_path("v1_config.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&content).unwrap();

    // V1 format has no version field
    assert!(config.get("version").is_none());
    // V1 format uses "remaps" instead of "profiles"
    assert!(config.get("remaps").is_some());
}
