//! Daemon service implementation

use anyhow::Result;
use std::collections::HashMap;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;
use tracing::{error, info, warn};

use crate::config::{ConfigManager, Profile};
use crate::core::{EngineState, RemapEngine};

/// Run as a daemon service
pub async fn run_daemon(profiles: Vec<Profile>) -> Result<()> {
    info!("Starting remapper daemon with {} profiles", profiles.len());

    // Set up signal handlers
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Create and start engines
    let mut engines: HashMap<String, RemapEngine> = HashMap::new();
    let mut handles = Vec::new();

    for profile in profiles {
        match RemapEngine::new(&profile).await {
            Ok(engine) => {
                info!("Created engine for profile: {}", profile.name);
                engines.insert(profile.name.clone(), engine);
            }
            Err(e) => {
                error!("Failed to create engine for {}: {}", profile.name, e);
            }
        }
    }

    if engines.is_empty() {
        error!("No profiles could be loaded");
        anyhow::bail!("No profiles could be loaded");
    }

    // Start all engines
    for (name, mut engine) in engines {
        let mut shutdown = shutdown_rx.clone();
        let handle = tokio::spawn(async move {
            info!("Starting engine: {}", name);

            // Run engine until shutdown
            tokio::select! {
                result = engine.start() => {
                    match result {
                        Ok(()) => info!("Engine {} stopped normally", name),
                        Err(e) => error!("Engine {} failed: {}", name, e),
                    }
                }
                _ = shutdown.changed() => {
                    info!("Engine {} received shutdown signal", name);
                }
            }

            // Stop engine
            if let Err(e) = engine.stop().await {
                error!("Error stopping engine {}: {}", name, e);
            }

            name
        });
        handles.push(handle);
    }

    info!("Daemon running with {} engines", handles.len());

    // Wait for signals
    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down");
                break;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down");
                break;
            }
            _ = sighup.recv() => {
                info!("Received SIGHUP, reloading configuration");
                // TODO: Implement config reload
                // For now, just log
                warn!("Config reload not yet implemented");
            }
        }
    }

    // Send shutdown signal
    info!("Sending shutdown signal to engines");
    let _ = shutdown_tx.send(true);

    // Wait for all engines to stop
    info!("Waiting for engines to stop...");
    for handle in handles {
        match handle.await {
            Ok(name) => info!("Engine {} stopped", name),
            Err(e) => error!("Error waiting for engine: {}", e),
        }
    }

    info!("Daemon shutdown complete");
    Ok(())
}

/// Daemon with automatic profile loading and hotplug support
pub async fn run_auto_daemon() -> Result<()> {
    info!("Starting remapper auto-daemon");

    // Load configuration
    let config = ConfigManager::load()?;
    let profiles: Vec<_> = config.enabled_profiles().into_iter().cloned().collect();

    if profiles.is_empty() {
        warn!("No enabled profiles found");
        return Ok(());
    }

    run_daemon(profiles).await
}

/// Status information for the daemon
#[derive(Debug)]
pub struct DaemonStatus {
    pub running: bool,
    pub uptime_secs: f64,
    pub engines: Vec<EngineStatus>,
}

/// Status of a single engine
#[derive(Debug)]
pub struct EngineStatus {
    pub name: String,
    pub state: EngineState,
    pub events_processed: u64,
    pub events_per_sec: f64,
}
