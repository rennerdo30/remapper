//! Daemon mode for running as a background service

mod connector;
mod ipc;
mod service;

pub use connector::{DaemonConnectionState, DaemonConnector};
pub use ipc::ProfileStatus;
pub use service::run_daemon_with_ipc;
