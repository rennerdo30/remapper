//! Device hotplug monitoring using inotify

use std::path::PathBuf;
use inotify::{EventMask, Inotify, WatchMask};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::core::error::Result;
use super::manager::{DeviceInfo, DeviceManager};

/// Device event types
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// A new device was connected
    Added(DeviceInfo),
    /// A device was disconnected
    Removed(PathBuf),
}

/// Monitors /dev/input for device changes
pub struct DeviceMonitor {
    /// Inotify instance
    inotify: Inotify,
    /// Event sender
    tx: mpsc::Sender<DeviceEvent>,
    /// Event receiver
    rx: mpsc::Receiver<DeviceEvent>,
}

impl DeviceMonitor {
    /// Create a new device monitor
    pub fn new() -> Result<Self> {
        let inotify = Inotify::init()?;

        // Watch /dev/input for create/delete events
        inotify.watches().add(
            "/dev/input",
            WatchMask::CREATE | WatchMask::DELETE,
        )?;

        let (tx, rx) = mpsc::channel(32);

        Ok(Self { inotify, tx, rx })
    }

    /// Start monitoring for device changes
    pub async fn start(mut self) -> mpsc::Receiver<DeviceEvent> {
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let mut buffer = [0u8; 4096];

            loop {
                match self.inotify.read_events(&mut buffer) {
                    Ok(events) => {
                        for event in events {
                            if let Some(name) = event.name.and_then(|n| n.to_str()) {
                                // Only care about event* devices
                                if !name.starts_with("event") {
                                    continue;
                                }

                                let path = PathBuf::from("/dev/input").join(name);

                                if event.mask.contains(EventMask::CREATE) {
                                    debug!("Device added: {}", path.display());

                                    // Wait a moment for device to be ready
                                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                                    if let Ok(Some(info)) = DeviceManager::find_by_path(&path) {
                                        info!("New device: {} ({})", info.name, info.path.display());
                                        let _ = tx.send(DeviceEvent::Added(info)).await;
                                    }
                                } else if event.mask.contains(EventMask::DELETE) {
                                    debug!("Device removed: {}", path.display());
                                    info!("Device disconnected: {}", path.display());
                                    let _ = tx.send(DeviceEvent::Removed(path)).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading inotify events: {}", e);
                        break;
                    }
                }

                // Small delay to prevent busy loop
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        });

        self.rx
    }
}

/// Watch for a specific device to be connected
pub async fn wait_for_device(name: &str, timeout: std::time::Duration) -> Result<Option<DeviceInfo>> {
    use tokio::time::timeout as tokio_timeout;

    // First check if already connected
    if let Some(info) = DeviceManager::find_by_name(name)? {
        return Ok(Some(info));
    }

    // Start monitoring
    let monitor = DeviceMonitor::new()?;
    let mut rx = monitor.start().await;
    let name_lower = name.to_lowercase();

    // Wait for device with timeout
    let result = tokio_timeout(timeout, async {
        while let Some(event) = rx.recv().await {
            if let DeviceEvent::Added(info) = event {
                if info.name.to_lowercase().contains(&name_lower) {
                    return Some(info);
                }
            }
        }
        None
    })
    .await;

    Ok(result.unwrap_or(None))
}
