//! Windows input device implementation using gilrs for gamepads

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

/// Windows input backend using gilrs for gamepads
pub struct WindowsInputBackend {
    gilrs: Arc<Mutex<Gilrs>>,
}

impl WindowsInputBackend {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().expect("Failed to initialize gilrs");
        Self {
            gilrs: Arc::new(Mutex::new(gilrs)),
        }
    }
}

impl Default for WindowsInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputBackend for WindowsInputBackend {
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
                supports_grab: false, // Windows doesn't support device grabbing like Linux
            };
            debug!("Found gamepad: {} ({})", info.name, info.id);
            devices.push(info);
        }

        // Note: Keyboard/mouse detection on Windows requires different APIs
        // For now, we focus on gamepad support via gilrs
        // Full keyboard/mouse support would need Raw Input API integration

        Ok(devices)
    }

    async fn open_device(&self, device_id: &str) -> Result<Box<dyn PlatformInputDevice>> {
        if device_id.starts_with("gamepad:") {
            let id_str = device_id.strip_prefix("gamepad:").unwrap_or("0");
            let id: usize = id_str.parse().map_err(|_| {
                RemapperError::DeviceNotFound(format!("Invalid gamepad ID: {}", device_id))
            })?;

            let device = WindowsInputDevice::open_gamepad(self.gilrs.clone(), id)?;
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

/// Windows input device using gilrs for gamepads
pub struct WindowsInputDevice {
    gilrs: Arc<Mutex<Gilrs>>,
    gamepad_id: gilrs::GamepadId,
    info: PlatformDeviceInfo,
    grabbed: bool,
}

impl WindowsInputDevice {
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
impl PlatformInputDevice for WindowsInputDevice {
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
        // Windows doesn't support true device grabbing like Linux
        // We just set a flag to indicate we're "exclusive"
        if !self.grabbed {
            warn!(
                "Device grabbing is not fully supported on Windows. \
                Other applications may still receive input from this device."
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
            // Standard gamepad buttons mapped to evdev-compatible codes
            304, 305, 306, 307, // BTN_SOUTH, EAST, C, NORTH (A, B, X, Y)
            308, 309,           // BTN_WEST, Z (LB, RB on Xbox)
            310, 311,           // BTN_TL, BTN_TR (LT, RT digital)
            312, 313,           // BTN_TL2, BTN_TR2
            314, 315,           // BTN_SELECT, BTN_START
            316, 317, 318,      // BTN_MODE, BTN_THUMBL, BTN_THUMBR
        ];

        let abs_axes = vec![
            AbsAxisInfo {
                code: 0,  // ABS_X - Left stick X
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 1,  // ABS_Y - Left stick Y
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 3,  // ABS_RX - Right stick X
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 4,  // ABS_RY - Right stick Y
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 2,  // ABS_Z - Left trigger
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 5,  // ABS_RZ - Right trigger
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 16, // ABS_HAT0X - D-pad X
                value: 0,
                minimum: -1,
                maximum: 1,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 17, // ABS_HAT0Y - D-pad Y
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
            Some(PlatformInputEvent::new(1, code, 1)) // EV_KEY, press
        }
        EventType::ButtonReleased(button, _) => {
            let code = button_to_code(button)?;
            Some(PlatformInputEvent::new(1, code, 0)) // EV_KEY, release
        }
        EventType::ButtonChanged(button, value, _) => {
            // For analog buttons like triggers
            let code = button_to_code(button)?;
            let int_value = (value * 255.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value)) // EV_ABS
        }
        EventType::AxisChanged(axis, value, _) => {
            let code = axis_to_code(axis)?;
            // Scale from -1.0..1.0 to -32768..32767
            let int_value = (value * 32767.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value)) // EV_ABS
        }
        EventType::Connected | EventType::Disconnected | EventType::Dropped => None,
        _ => None, // Handle any future EventType variants
    }
}

/// Convert gilrs Button to evdev-compatible code
fn button_to_code(button: Button) -> Option<u16> {
    match button {
        Button::South => Some(304),      // BTN_SOUTH / BTN_A
        Button::East => Some(305),       // BTN_EAST / BTN_B
        Button::North => Some(307),      // BTN_NORTH / BTN_X
        Button::West => Some(308),       // BTN_WEST / BTN_Y
        Button::LeftTrigger => Some(310),  // BTN_TL
        Button::RightTrigger => Some(311), // BTN_TR
        Button::LeftTrigger2 => Some(312), // BTN_TL2
        Button::RightTrigger2 => Some(313), // BTN_TR2
        Button::Select => Some(314),     // BTN_SELECT
        Button::Start => Some(315),      // BTN_START
        Button::Mode => Some(316),       // BTN_MODE
        Button::LeftThumb => Some(317),  // BTN_THUMBL
        Button::RightThumb => Some(318), // BTN_THUMBR
        Button::DPadUp => Some(0),       // Use HAT events instead
        Button::DPadDown => Some(0),
        Button::DPadLeft => Some(0),
        Button::DPadRight => Some(0),
        Button::C => Some(306),          // BTN_C
        Button::Z => Some(309),          // BTN_Z
        Button::Unknown => None,
    }
}

/// Convert gilrs Axis to evdev-compatible code
fn axis_to_code(axis: Axis) -> Option<u16> {
    match axis {
        Axis::LeftStickX => Some(0),   // ABS_X
        Axis::LeftStickY => Some(1),   // ABS_Y
        Axis::RightStickX => Some(3),  // ABS_RX
        Axis::RightStickY => Some(4),  // ABS_RY
        Axis::LeftZ => Some(2),        // ABS_Z (left trigger)
        Axis::RightZ => Some(5),       // ABS_RZ (right trigger)
        Axis::DPadX => Some(16),       // ABS_HAT0X
        Axis::DPadY => Some(17),       // ABS_HAT0Y
        Axis::Unknown => None,
    }
}
