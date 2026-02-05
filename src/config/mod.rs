//! Configuration management

mod manager;
mod migration;
pub mod schema;

pub use manager::ConfigManager;
pub use schema::{DeviceMatch, ExecutionMode, MacroStep, Mapping, OutputConfig, Profile};
