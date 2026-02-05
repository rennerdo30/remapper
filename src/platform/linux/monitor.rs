//! Linux device hotplug monitoring using inotify

use std::path::PathBuf;

use async_trait::async_trait;
use inotify::{EventMask, Inotify, WatchMask};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::core::error::Result;
use crate::platform::traits::{DeviceChangeEvent, DeviceMonitor, InputBackend};

use super::LinuxInputBackend;

/// Linux device monitor using inotify
pub struct LinuxDeviceMonitor {
    /// Inotify instance
    inotify: Inotify,
}

impl LinuxDeviceMonitor {
    /// Create a new device monitor
    pub fn new() -> Result<Self> {
        let inotify = Inotify::init()?;

        // Watch /dev/input for create/delete events
        inotify
            .watches()
            .add("/dev/input", WatchMask::CREATE | WatchMask::DELETE)?;

        Ok(Self { inotify })
    }
}

#[async_trait]
impl DeviceMonitor for LinuxDeviceMonitor {
    async fn start(mut self) -> mpsc::Receiver<DeviceChangeEvent> {
        let (tx, rx) = mpsc::channel(32);
        let backend = LinuxInputBackend::new();

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
                                let device_id = path.display().to_string();

                                if event.mask.contains(EventMask::CREATE) {
                                    debug!("Device added: {}", path.display());

                                    // Wait a moment for device to be ready
                                    tokio::time::sleep(std::time::Duration::from_millis(100))
                                        .await;

                                    // Refresh device list and find the just-created path.
                                    if let Ok(devices) = backend.list_devices().await {
                                        if let Some(info) =
                                            devices.into_iter().find(|d| d.id == device_id)
                                        {
                                            info!("New device: {} ({})", info.name, info.id);
                                            let _ = tx.send(DeviceChangeEvent::Added(info)).await;
                                        }
                                    } else {
                                        // Best-effort fallback when list refresh fails.
                                        if let Ok(Some(info)) = backend.find_by_name(name).await {
                                            info!("New device: {} ({})", info.name, info.id);
                                            let _ = tx.send(DeviceChangeEvent::Added(info)).await;
                                        }
                                    }
                                } else if event.mask.contains(EventMask::DELETE) {
                                    debug!("Device removed: {}", path.display());
                                    info!("Device disconnected: {}", path.display());
                                    let _ = tx.send(DeviceChangeEvent::Removed(device_id)).await;
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

        rx
    }
}
