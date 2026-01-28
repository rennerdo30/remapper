//! Core remapping engine

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::Profile;
use crate::devices::{InputDevice, OutputDevice};
use crate::mappings::MappingHandler;

use super::error::{RemapperError, Result};
use super::events::InputEvent;

/// Engine state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Engine is stopped
    Stopped,
    /// Engine is starting up
    Starting,
    /// Engine is running and processing events
    Running,
    /// Engine is shutting down
    Stopping,
    /// Engine encountered an error
    Error,
}

impl std::fmt::Display for EngineState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineState::Stopped => write!(f, "Stopped"),
            EngineState::Starting => write!(f, "Starting"),
            EngineState::Running => write!(f, "Running"),
            EngineState::Stopping => write!(f, "Stopping"),
            EngineState::Error => write!(f, "Error"),
        }
    }
}

/// Engine statistics
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    /// Number of events received
    pub events_received: u64,
    /// Number of events remapped
    pub events_remapped: u64,
    /// Number of events passed through
    pub events_passthrough: u64,
    /// Number of events dropped
    pub events_dropped: u64,
    /// Engine start time
    pub started_at: Option<std::time::Instant>,
    /// Last event time
    pub last_event_at: Option<std::time::Instant>,
}

impl EngineStats {
    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> f64 {
        self.started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Get events per second
    pub fn events_per_sec(&self) -> f64 {
        let uptime = self.uptime_secs();
        if uptime > 0.0 {
            self.events_received as f64 / uptime
        } else {
            0.0
        }
    }
}

/// Remapping engine
pub struct RemapEngine {
    /// Profile name
    name: String,
    /// Input device
    input: Arc<Mutex<InputDevice>>,
    /// Output device
    output: Arc<Mutex<OutputDevice>>,
    /// Mapping handlers
    handlers: Vec<Box<dyn MappingHandler>>,
    /// Current state
    state: Arc<RwLock<EngineState>>,
    /// Statistics
    stats: Arc<RwLock<EngineStats>>,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
    /// Shutdown signal receiver
    shutdown_rx: watch::Receiver<bool>,
    /// Whether to grab the input device
    grab: bool,
}

impl RemapEngine {
    /// Create a new remapping engine from a profile
    pub async fn new(profile: &Profile) -> Result<Self> {
        info!("Creating engine for profile: {}", profile.name);

        // Open input device
        let input_path = profile.input_device.resolve()?;
        let input = InputDevice::open(&input_path).await?;
        info!("Opened input device: {}", input_path.display());

        // Create output device with capabilities from input
        let output_name = profile
            .output_device
            .name
            .clone()
            .unwrap_or_else(|| format!("Remapped {}", profile.name));
        let output = OutputDevice::create(&output_name, &input)?;
        info!("Created output device: {}", output_name);

        // Build mapping handlers
        let handlers = Self::build_handlers(&profile.mappings)?;
        info!("Created {} mapping handlers", handlers.len());

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Ok(Self {
            name: profile.name.clone(),
            input: Arc::new(Mutex::new(input)),
            output: Arc::new(Mutex::new(output)),
            handlers,
            state: Arc::new(RwLock::new(EngineState::Stopped)),
            stats: Arc::new(RwLock::new(EngineStats::default())),
            shutdown_tx,
            shutdown_rx,
            grab: profile.grab,
        })
    }

    /// Build mapping handlers from configuration
    fn build_handlers(
        mappings: &[crate::config::Mapping],
    ) -> Result<Vec<Box<dyn MappingHandler>>> {
        use crate::mappings::{ComboHandler, ConditionalHandler, MacroHandler, SimpleHandler};

        let mut handlers: Vec<Box<dyn MappingHandler>> = Vec::new();

        for mapping in mappings {
            let handler: Box<dyn MappingHandler> = match mapping {
                crate::config::Mapping::Simple { from, to } => {
                    Box::new(SimpleHandler::new(from.clone(), to.clone())?)
                }
                crate::config::Mapping::Macro { trigger, sequence } => {
                    Box::new(MacroHandler::new(trigger.clone(), sequence.clone())?)
                }
                crate::config::Mapping::Conditional {
                    trigger,
                    tap,
                    hold,
                    threshold_ms,
                } => Box::new(ConditionalHandler::new(
                    trigger.clone(),
                    tap.clone(),
                    hold.clone(),
                    *threshold_ms,
                )?),
                crate::config::Mapping::Combo {
                    keys,
                    output,
                    order_sensitive,
                } => Box::new(ComboHandler::new(
                    keys.clone(),
                    output.clone(),
                    *order_sensitive,
                )?),
            };
            handlers.push(handler);
        }

        Ok(handlers)
    }

    /// Get engine name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get current state
    pub async fn state(&self) -> EngineState {
        *self.state.read().await
    }

    /// Get current statistics
    pub async fn stats(&self) -> EngineStats {
        self.stats.read().await.clone()
    }

    /// Start the engine
    pub async fn start(&mut self) -> Result<()> {
        let current_state = self.state().await;
        if current_state == EngineState::Running {
            return Err(RemapperError::EngineAlreadyRunning(self.name.clone()));
        }

        *self.state.write().await = EngineState::Starting;
        info!("Starting engine: {}", self.name);

        // Grab input device if configured
        if self.grab {
            let mut input = self.input.lock().await;
            input.grab().await?;
            info!("Grabbed input device");
        }

        // Initialize stats
        {
            let mut stats = self.stats.write().await;
            *stats = EngineStats::default();
            stats.started_at = Some(std::time::Instant::now());
        }

        *self.state.write().await = EngineState::Running;

        // Run event loop
        self.run_event_loop().await
    }

    /// Run the main event loop
    async fn run_event_loop(&mut self) -> Result<()> {
        let input = Arc::clone(&self.input);
        let output = Arc::clone(&self.output);
        let state = Arc::clone(&self.state);
        let stats = Arc::clone(&self.stats);
        let mut shutdown_rx = self.shutdown_rx.clone();

        loop {
            // Check for shutdown signal
            if *shutdown_rx.borrow() {
                info!("Shutdown signal received");
                break;
            }

            // Read event from input device
            let event = {
                let mut input_guard = input.lock().await;
                tokio::select! {
                    result = input_guard.read_event() => result,
                    _ = shutdown_rx.changed() => {
                        info!("Shutdown during event read");
                        break;
                    }
                }
            };

            match event {
                Ok(Some(ev)) => {
                    // Update stats
                    {
                        let mut s = stats.write().await;
                        s.events_received += 1;
                        s.last_event_at = Some(std::time::Instant::now());
                    }

                    // Process event through handlers
                    let output_events = self.process_event(ev).await;

                    // Write output events
                    let output_guard = output.lock().await;
                    for out_ev in output_events {
                        if let Err(e) = output_guard.write_event(&out_ev) {
                            error!("Failed to write event: {}", e);
                            let mut s = stats.write().await;
                            s.events_dropped += 1;
                        }
                    }
                    if let Err(e) = output_guard.sync() {
                        error!("Failed to sync output device: {}", e);
                    }
                }
                Ok(None) => {
                    // No event available, yield
                    tokio::task::yield_now().await;
                }
                Err(e) => {
                    error!("Error reading event: {}", e);
                    *state.write().await = EngineState::Error;
                    return Err(e);
                }
            }
        }

        *self.state.write().await = EngineState::Stopped;
        Ok(())
    }

    /// Process an input event through handlers
    async fn process_event(&mut self, event: InputEvent) -> Vec<InputEvent> {
        // Skip sync events - pass through directly
        if event.is_sync() {
            return vec![event];
        }

        // Try each handler
        for handler in &mut self.handlers {
            if handler.handles(&event) {
                debug!("Handler matched event: {}", event);
                let output = handler.process(event.clone()).await;
                {
                    let mut s = self.stats.write().await;
                    s.events_remapped += 1;
                }
                return output;
            }
        }

        // No handler matched - pass through
        {
            let mut s = self.stats.write().await;
            s.events_passthrough += 1;
        }
        vec![event]
    }

    /// Stop the engine
    pub async fn stop(&mut self) -> Result<()> {
        let current_state = self.state().await;
        if current_state != EngineState::Running {
            return Ok(());
        }

        info!("Stopping engine: {}", self.name);
        *self.state.write().await = EngineState::Stopping;

        // Send shutdown signal
        let _ = self.shutdown_tx.send(true);

        // Ungrab input device
        if self.grab {
            let mut input = self.input.lock().await;
            if let Err(e) = input.ungrab().await {
                warn!("Failed to ungrab device: {}", e);
            }
        }

        // Reset handlers
        for handler in &mut self.handlers {
            handler.reset();
        }

        *self.state.write().await = EngineState::Stopped;
        info!("Engine stopped: {}", self.name);
        Ok(())
    }
}

impl Drop for RemapEngine {
    fn drop(&mut self) {
        // Send shutdown signal on drop
        let _ = self.shutdown_tx.send(true);
    }
}

/// Manager for multiple engines
pub struct EngineManager {
    /// Running engines by profile name
    engines: HashMap<String, RemapEngine>,
}

impl EngineManager {
    /// Create a new engine manager
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    /// Start a profile
    pub async fn start_profile(&mut self, profile: Profile) -> Result<()> {
        if self.engines.contains_key(&profile.name) {
            return Err(RemapperError::EngineAlreadyRunning(profile.name.clone()));
        }

        let mut engine = RemapEngine::new(&profile).await?;
        engine.start().await?;
        self.engines.insert(profile.name.clone(), engine);
        Ok(())
    }

    /// Stop a profile
    pub async fn stop_profile(&mut self, name: &str) -> Result<()> {
        if let Some(mut engine) = self.engines.remove(name) {
            engine.stop().await?;
        }
        Ok(())
    }

    /// Stop all profiles
    pub async fn stop_all(&mut self) -> Result<()> {
        for (_, mut engine) in self.engines.drain() {
            let _ = engine.stop().await;
        }
        Ok(())
    }

    /// Get engine states
    pub async fn states(&self) -> HashMap<String, EngineState> {
        let mut states = HashMap::new();
        for (name, engine) in &self.engines {
            states.insert(name.clone(), engine.state().await);
        }
        states
    }

    /// Get engine statistics
    pub async fn stats(&self) -> HashMap<String, EngineStats> {
        let mut all_stats = HashMap::new();
        for (name, engine) in &self.engines {
            all_stats.insert(name.clone(), engine.stats().await);
        }
        all_stats
    }
}

impl Default for EngineManager {
    fn default() -> Self {
        Self::new()
    }
}
