//! macOS input device implementation using gilrs for gamepads

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use gilrs::{ev::Axis, ev::Button, Event, EventType, Gilrs};
use tracing::{debug, trace, warn};

use crate::core::error::{RemapperError, Result};
use crate::platform::traits::{
    AbsAxisInfo, DeviceCapabilities, DeviceType, InputBackend, PlatformDeviceInfo,
    PlatformInputDevice, PlatformInputEvent,
};

/// macOS input backend using gilrs for gamepads
pub struct MacOSInputBackend {
    gilrs: Arc<Mutex<Gilrs>>,
}

impl MacOSInputBackend {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().expect("Failed to initialize gilrs");
        Self {
            gilrs: Arc::new(Mutex::new(gilrs)),
        }
    }
}

impl Default for MacOSInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputBackend for MacOSInputBackend {
    async fn list_devices(&self) -> Result<Vec<PlatformDeviceInfo>> {
        let gilrs = self.gilrs.lock().map_err(|_| {
            RemapperError::DeviceNotFound("Failed to lock gilrs".to_string())
        })?;

        let mut devices = Vec::new();

        for (id, gamepad) in gilrs.gamepads() {
            let info = PlatformDeviceInfo {
                id: format!("gamepad:{}", usize::from(id)),
                name: gamepad.name().to_string(),
                vendor_id: gamepad.vendor_id().unwrap_or(0),
                product_id: gamepad.product_id().unwrap_or(0),
                device_type: DeviceType::Gamepad,
                path: None,
                supports_grab: false, // macOS doesn't support device grabbing
            };
            debug!("Found gamepad: {} ({})", info.name, info.id);
            devices.push(info);
        }

        // Note: Full keyboard/mouse detection on macOS requires IOKit HID Manager
        // which needs additional permissions (Input Monitoring in Privacy settings)

        Ok(devices)
    }

    async fn open_device(&self, device_id: &str) -> Result<Box<dyn PlatformInputDevice>> {
        if device_id.starts_with("gamepad:") {
            let id_str = device_id.strip_prefix("gamepad:").unwrap_or("0");
            let id: usize = id_str.parse().map_err(|_| {
                RemapperError::DeviceNotFound(format!("Invalid gamepad ID: {}", device_id))
            })?;

            let device = MacOSInputDevice::open_gamepad(self.gilrs.clone(), id)?;
            Ok(Box::new(device))
        } else {
            Err(RemapperError::DeviceNotFound(format!(
                "Unknown device type: {}",
                device_id
            )))
        }
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<PlatformDeviceInfo>> {
        let devices = self.list_devices().await?;
        let name_lower = name.to_lowercase();
        Ok(devices
            .into_iter()
            .find(|d| d.name.to_lowercase().contains(&name_lower)))
    }

    async fn find_by_id(&self, vendor: u16, product: u16) -> Result<Option<PlatformDeviceInfo>> {
        let devices = self.list_devices().await?;
        Ok(devices
            .into_iter()
            .find(|d| d.vendor_id == vendor && d.product_id == product))
    }
}

/// macOS input device using gilrs for gamepads
pub struct MacOSInputDevice {
    gilrs: Arc<Mutex<Gilrs>>,
    gamepad_id: gilrs::GamepadId,
    info: PlatformDeviceInfo,
    grabbed: bool,
}

impl MacOSInputDevice {
    /// Open a gamepad device by index
    pub fn open_gamepad(gilrs: Arc<Mutex<Gilrs>>, index: usize) -> Result<Self> {
        let gilrs_guard = gilrs.lock().map_err(|_| {
            RemapperError::DeviceNotFound("Failed to lock gilrs".to_string())
        })?;

        let (gamepad_id, gamepad) = gilrs_guard
            .gamepads()
            .nth(index)
            .ok_or_else(|| RemapperError::DeviceNotFound(format!("Gamepad {} not found", index)))?;

        let info = PlatformDeviceInfo {
            id: format!("gamepad:{}", usize::from(gamepad_id)),
            name: gamepad.name().to_string(),
            vendor_id: gamepad.vendor_id().unwrap_or(0),
            product_id: gamepad.product_id().unwrap_or(0),
            device_type: DeviceType::Gamepad,
            path: None,
            supports_grab: false,
        };

        debug!("Opened gamepad: {} ({})", info.name, info.id);

        drop(gilrs_guard);

        Ok(Self {
            gilrs,
            gamepad_id,
            info,
            grabbed: false,
        })
    }
}

#[async_trait]
impl PlatformInputDevice for MacOSInputDevice {
    async fn read_event(&mut self) -> Result<Option<PlatformInputEvent>> {
        // Poll for events with a small sleep to prevent busy-waiting
        tokio::time::sleep(Duration::from_millis(1)).await;

        let mut gilrs = self.gilrs.lock().map_err(|_| {
            RemapperError::EventReadError("Failed to lock gilrs".to_string())
        })?;

        // Process next event
        while let Some(event) = gilrs.next_event() {
            if event.id != self.gamepad_id {
                continue;
            }

            if let Some(platform_event) = gilrs_event_to_platform(&event) {
                trace!("Read event: {:?}", platform_event);
                return Ok(Some(platform_event));
            }
        }

        Ok(None)
    }

    async fn grab(&mut self) -> Result<()> {
        // macOS doesn't support device grabbing
        if !self.grabbed {
            warn!(
                "Device grabbing is not supported on macOS. \
                Other applications will still receive input from this device."
            );
            self.grabbed = true;
        }
        Ok(())
    }

    async fn ungrab(&mut self) -> Result<()> {
        self.grabbed = false;
        Ok(())
    }

    fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    fn info(&self) -> &PlatformDeviceInfo {
        &self.info
    }

    fn capabilities(&self) -> DeviceCapabilities {
        // Return standard gamepad capabilities
        let keys = vec![
            304, 305, 306, 307, // BTN_SOUTH, EAST, C, NORTH
            308, 309,           // BTN_WEST, Z
            310, 311,           // BTN_TL, BTN_TR
            312, 313,           // BTN_TL2, BTN_TR2
            314, 315,           // BTN_SELECT, BTN_START
            316, 317, 318,      // BTN_MODE, BTN_THUMBL, BTN_THUMBR
        ];

        let abs_axes = vec![
            AbsAxisInfo {
                code: 0,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 1,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 3,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 4,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 2,
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 5,
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 16,
                value: 0,
                minimum: -1,
                maximum: 1,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 17,
                value: 0,
                minimum: -1,
                maximum: 1,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        ];

        DeviceCapabilities {
            keys,
            abs_axes,
            rel_axes: vec![],
        }
    }
}

/// Convert gilrs event to platform event
fn gilrs_event_to_platform(event: &Event) -> Option<PlatformInputEvent> {
    match event.event {
        EventType::ButtonPressed(button, _) => {
            let code = button_to_code(button)?;
            Some(PlatformInputEvent::new(1, code, 1))
        }
        EventType::ButtonReleased(button, _) => {
            let code = button_to_code(button)?;
            Some(PlatformInputEvent::new(1, code, 0))
        }
        EventType::ButtonChanged(button, value, _) => {
            let code = button_to_code(button)?;
            let int_value = (value * 255.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value))
        }
        EventType::AxisChanged(axis, value, _) => {
            let code = axis_to_code(axis)?;
            let int_value = (value * 32767.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value))
        }
        EventType::Connected | EventType::Disconnected | EventType::Dropped => None,
        _ => None, // Handle any future EventType variants
    }
}

fn button_to_code(button: Button) -> Option<u16> {
    match button {
        Button::South => Some(304),
        Button::East => Some(305),
        Button::North => Some(307),
        Button::West => Some(308),
        Button::LeftTrigger => Some(310),
        Button::RightTrigger => Some(311),
        Button::LeftTrigger2 => Some(312),
        Button::RightTrigger2 => Some(313),
        Button::Select => Some(314),
        Button::Start => Some(315),
        Button::Mode => Some(316),
        Button::LeftThumb => Some(317),
        Button::RightThumb => Some(318),
        Button::C => Some(306),
        Button::Z => Some(309),
        Button::DPadUp | Button::DPadDown | Button::DPadLeft | Button::DPadRight => Some(0),
        Button::Unknown => None,
    }
}

fn axis_to_code(axis: Axis) -> Option<u16> {
    match axis {
        Axis::LeftStickX => Some(0),
        Axis::LeftStickY => Some(1),
        Axis::RightStickX => Some(3),
        Axis::RightStickY => Some(4),
        Axis::LeftZ => Some(2),
        Axis::RightZ => Some(5),
        Axis::DPadX => Some(16),
        Axis::DPadY => Some(17),
        Axis::Unknown => None,
    }
}
