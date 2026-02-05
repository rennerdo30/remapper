//! Daemon service implementation

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::{ConfigManager, Profile};
use crate::core::{EngineState, RemapEngine};

use super::ipc::{IpcConnection, IpcRequest, IpcResponse, IpcServer, ProfileStatus};

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

/// Running engine with its control handles
struct RunningEngine {
    engine: Arc<Mutex<RemapEngine>>,
    shutdown_tx: watch::Sender<bool>,
    state: Arc<RwLock<EngineState>>,
    start_time: Instant,
    events_processed: Arc<RwLock<u64>>,
}

/// Shared daemon state for IPC handlers
struct DaemonState {
    config: Arc<RwLock<ConfigManager>>,
    engines: Arc<RwLock<HashMap<String, RunningEngine>>>,
    start_time: Instant,
    shutdown_tx: watch::Sender<bool>,
}

impl DaemonState {
    /// Start a profile by name
    async fn start_profile(&self, name: &str) -> Result<String, String> {
        // Check if already running
        {
            let engines = self.engines.read().await;
            if engines.contains_key(name) {
                return Err(format!("Profile '{}' is already running", name));
            }
        }

        // Get profile from config
        let profile = {
            let config = self.config.read().await;
            config
                .get_profile(name)
                .cloned()
                .ok_or_else(|| format!("Profile '{}' not found", name))?
        };

        // Create engine
        let engine = RemapEngine::new(&profile)
            .await
            .map_err(|e| format!("Failed to create engine: {}", e))?;

        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let state = Arc::new(RwLock::new(EngineState::Starting));
        let events_processed = Arc::new(RwLock::new(0u64));

        let running = RunningEngine {
            engine: Arc::new(Mutex::new(engine)),
            shutdown_tx,
            state: state.clone(),
            start_time: Instant::now(),
            events_processed: events_processed.clone(),
        };

        // Store the engine
        {
            let mut engines = self.engines.write().await;
            engines.insert(name.to_string(), running);
        }

        // Spawn the engine task
        let engine_arc = {
            let engines = self.engines.read().await;
            engines.get(name).unwrap().engine.clone()
        };
        let state_clone = state.clone();
        let name_clone = name.to_string();
        let engines_ref = self.engines.clone();

        tokio::spawn(async move {
            // Update state to running
            {
                let mut s = state_clone.write().await;
                *s = EngineState::Running;
            }

            // Run the engine
            let result = {
                let mut engine = engine_arc.lock().await;
                tokio::select! {
                    result = engine.start() => result,
                    _ = shutdown_rx.changed() => {
                        debug!("Engine {} received shutdown signal", name_clone);
                        Ok(())
                    }
                }
            };

            // Update state based on result
            {
                let mut s = state_clone.write().await;
                match result {
                    Ok(()) => *s = EngineState::Stopped,
                    Err(e) => {
                        error!("Engine {} failed: {}", name_clone, e);
                        *s = EngineState::Error;
                    }
                }
            }

            // Stop the engine
            {
                let mut engine = engine_arc.lock().await;
                if let Err(e) = engine.stop().await {
                    error!("Error stopping engine {}: {}", name_clone, e);
                }
            }

            // Remove from running engines
            {
                let mut engines = engines_ref.write().await;
                engines.remove(&name_clone);
            }

            info!("Engine {} stopped", name_clone);
        });

        Ok(format!("Profile '{}' started", name))
    }

    /// Stop a profile by name
    async fn stop_profile(&self, name: &str) -> Result<String, String> {
        let engines = self.engines.read().await;
        if let Some(engine) = engines.get(name) {
            let _ = engine.shutdown_tx.send(true);
            Ok(format!("Stopping profile '{}'", name))
        } else {
            Err(format!("Profile '{}' is not running", name))
        }
    }

    /// Get list of running profiles
    async fn list_running(&self) -> Vec<ProfileStatus> {
        let engines = self.engines.read().await;
        let mut profiles = Vec::new();

        for (name, engine) in engines.iter() {
            let state = *engine.state.read().await;
            let events = *engine.events_processed.read().await;
            let uptime = engine.start_time.elapsed().as_secs_f64();

            profiles.push(ProfileStatus {
                name: name.clone(),
                state,
                events_processed: events,
                uptime_secs: uptime,
            });
        }

        profiles
    }

    /// Get daemon status
    async fn status(&self) -> IpcResponse {
        let profiles = self.list_running().await;
        IpcResponse::Status {
            running: true,
            uptime_secs: self.start_time.elapsed().as_secs_f64(),
            profiles,
        }
    }
}

/// Handle a single IPC connection
async fn handle_ipc_connection(mut conn: IpcConnection, state: Arc<DaemonState>) {
    loop {
        match conn.read_request().await {
            Ok(Some(request)) => {
                debug!("Received IPC request: {:?}", request);

                let response = match request {
                    IpcRequest::Ping => IpcResponse::Pong,

                    IpcRequest::Status => state.status().await,

                    IpcRequest::StartProfile { ref name } => {
                        match state.start_profile(name).await {
                            Ok(msg) => IpcResponse::Ok { message: msg },
                            Err(msg) => IpcResponse::Error { message: msg },
                        }
                    }

                    IpcRequest::StopProfile { ref name } => match state.stop_profile(name).await {
                        Ok(msg) => IpcResponse::Ok { message: msg },
                        Err(msg) => IpcResponse::Error { message: msg },
                    },

                    IpcRequest::ListRunning => {
                        let profiles = state.list_running().await;
                        IpcResponse::RunningProfiles { profiles }
                    }

                    IpcRequest::ReloadConfig => {
                        let mut config = state.config.write().await;
                        match config.reload() {
                            Ok(()) => IpcResponse::Ok {
                                message: "Configuration reloaded".to_string(),
                            },
                            Err(e) => IpcResponse::Error {
                                message: format!("Failed to reload config: {}", e),
                            },
                        }
                    }

                    IpcRequest::Shutdown => {
                        info!("Received shutdown request via IPC");
                        let _ = state.shutdown_tx.send(true);
                        IpcResponse::Ok {
                            message: "Daemon shutting down".to_string(),
                        }
                    }
                };

                if let Err(e) = conn.send_response(&response).await {
                    error!("Failed to send IPC response: {}", e);
                    break;
                }

                // Exit loop after shutdown request
                if matches!(request, IpcRequest::Shutdown) {
                    break;
                }
            }
            Ok(None) => {
                // Connection closed
                debug!("IPC connection closed");
                break;
            }
            Err(e) => {
                error!("Error reading IPC request: {}", e);
                break;
            }
        }
    }
}

/// Run daemon with IPC support
///
/// This version of the daemon listens on a Unix socket for control commands
/// from the GUI or CLI.
pub async fn run_daemon_with_ipc() -> Result<()> {
    info!("Starting remapper daemon with IPC support");

    // Load configuration
    let config = ConfigManager::load()?;

    // Set up signal handlers
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;

    // Create shutdown channel
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    // Create IPC server
    let ipc_server = IpcServer::new().await?;
    info!("IPC server listening at {:?}", ipc_server.path());

    // Create shared daemon state
    let state = Arc::new(DaemonState {
        config: Arc::new(RwLock::new(config)),
        engines: Arc::new(RwLock::new(HashMap::new())),
        start_time: Instant::now(),
        shutdown_tx: shutdown_tx.clone(),
    });

    // Auto-start enabled profiles if configured
    {
        let config = state.config.read().await;
        if config.config().settings.auto_start {
            for profile in config.enabled_profiles() {
                info!("Auto-starting profile: {}", profile.name);
                if let Err(e) = state.start_profile(&profile.name).await {
                    error!("Failed to auto-start {}: {}", profile.name, e);
                }
            }
        }
    }

    // Main event loop
    loop {
        tokio::select! {
            // Accept new IPC connections
            result = ipc_server.accept() => {
                match result {
                    Ok(conn) => {
                        let state_clone = state.clone();
                        tokio::spawn(async move {
                            handle_ipc_connection(conn, state_clone).await;
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept IPC connection: {}", e);
                    }
                }
            }

            // Handle SIGTERM
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down");
                break;
            }

            // Handle SIGINT
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down");
                break;
            }

            // Handle SIGHUP (reload config)
            _ = sighup.recv() => {
                info!("Received SIGHUP, reloading configuration");
                let mut config = state.config.write().await;
                if let Err(e) = config.reload() {
                    error!("Failed to reload config: {}", e);
                }
            }

            // Check for shutdown signal from IPC
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Shutdown signal received");
                    break;
                }
            }
        }
    }

    // Send shutdown signal to all engines
    info!("Sending shutdown signal to all engines");
    {
        let engines = state.engines.read().await;
        for (name, engine) in engines.iter() {
            info!("Stopping engine: {}", name);
            let _ = engine.shutdown_tx.send(true);
        }
    }

    // Wait a moment for engines to stop
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    info!("Daemon shutdown complete");
    Ok(())
}
