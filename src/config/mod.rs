//! Configuration management

mod manager;
mod migration;
mod schema;

pub use manager::ConfigManager;
pub use schema::{DeviceMatch, MacroStep, Mapping, OutputConfig, Profile};
