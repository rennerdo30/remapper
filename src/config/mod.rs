//! Configuration management

mod manager;
mod migration;
mod schema;

pub use manager::ConfigManager;
pub use schema::{
    Config, DeviceMatch, MacroStep, Mapping, OutputConfig, Profile, Settings,
};
