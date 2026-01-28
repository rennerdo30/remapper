//! macOS device hotplug monitoring

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use gilrs::{EventType, Gilrs};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::core::error::Result;
use crate::platform::traits::{DeviceChangeEvent, DeviceMonitor, DeviceType, PlatformDeviceInfo};

/// macOS device monitor using gilrs for gamepad hotplug detection
pub struct MacOSDeviceMonitor {
    gilrs: Arc<Mutex<Gilrs>>,
}

impl MacOSDeviceMonitor {
    pub fn new() -> Result<Self> {
        let gilrs = Gilrs::new()
            .map_err(|e| crate::core::error::RemapperError::DeviceNotFound(e.to_string()))?;

        Ok(Self {
            gilrs: Arc::new(Mutex::new(gilrs)),
        })
    }
}

#[async_trait]
impl DeviceMonitor for MacOSDeviceMonitor {
    async fn start(self) -> mpsc::Receiver<DeviceChangeEvent> {
        let (tx, rx) = mpsc::channel(32);
        let gilrs = self.gilrs;

        tokio::spawn(async move {
            loop {
                // Collect events while holding the lock, then release before async send
                let events: Vec<DeviceChangeEvent> = {
                    let mut gilrs_guard = match gilrs.lock() {
                        Ok(g) => g,
                        Err(e) => {
                            error!("Failed to lock gilrs: {}", e);
                            break;
                        }
                    };

                    let mut collected = Vec::new();
                    while let Some(event) = gilrs_guard.next_event() {
                        match event.event {
                            EventType::Connected => {
                                if let Some(gamepad) = gilrs_guard.connected_gamepad(event.id) {
                                    let info = PlatformDeviceInfo {
                                        id: format!("gamepad:{}", usize::from(event.id)),
                                        name: gamepad.name().to_string(),
                                        vendor_id: gamepad.vendor_id().unwrap_or(0),
                                        product_id: gamepad.product_id().unwrap_or(0),
                                        device_type: DeviceType::Gamepad,
                                        path: None,
                                        supports_grab: false,
                                    };
                                    info!("Gamepad connected: {} ({})", info.name, info.id);
                                    collected.push(DeviceChangeEvent::Added(info));
                                }
                            }
                            EventType::Disconnected => {
                                let device_id = format!("gamepad:{}", usize::from(event.id));
                                info!("Gamepad disconnected: {}", device_id);
                                collected.push(DeviceChangeEvent::Removed(device_id));
                            }
                            _ => {}
                        }
                    }
                    collected
                }; // gilrs_guard is dropped here

                // Now send events without holding the lock
                for event in events {
                    let _ = tx.send(event).await;
                }

                // Sleep to prevent busy-waiting
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        rx
    }
}
