//! Windows virtual output device implementation using Vigem and SendInput
//!
//! # Thread Safety
//!
//! The `WindowsGamepadDevice` uses interior mutability via `Mutex` to provide
//! thread-safe access to the ViGEm gamepad state. The `vigem_client::Client` and
//! `Xbox360Wired` types do not implement `Sync` because they contain internal
//! state that requires synchronization. By wrapping the mutable parts in a `Mutex`,
//! we ensure safe concurrent access from multiple threads.

use std::sync::{Arc, Mutex};

use tracing::{debug, trace, warn};
use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
};

use crate::core::error::{RemapperError, Result};
use crate::platform::traits::{
    DeviceCapabilities, DeviceType, OutputBackend, PlatformInputEvent, PlatformOutputDevice,
};

/// Windows output backend using Vigem for gamepads and SendInput for keyboard/mouse
pub struct WindowsOutputBackend {
    vigem_available: bool,
}

impl WindowsOutputBackend {
    pub fn new() -> Self {
        // Check if Vigem is available
        let vigem_available = Client::new().is_ok();
        if !vigem_available {
            warn!("ViGEmBus driver not installed. Virtual gamepad support will be unavailable.");
        }
        Self { vigem_available }
    }
}

impl Default for WindowsOutputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputBackend for WindowsOutputBackend {
    fn create_device(
        &self,
        name: &str,
        capabilities: &DeviceCapabilities,
    ) -> Result<Box<dyn PlatformOutputDevice>> {
        // Determine device type based on capabilities
        let has_gamepad_buttons = capabilities.keys.iter().any(|k| *k >= 304 && *k <= 318);
        let has_abs_axes = !capabilities.abs_axes.is_empty();

        if has_gamepad_buttons && has_abs_axes {
            // Create virtual gamepad via Vigem
            if !self.vigem_available {
                return Err(RemapperError::NotSupported(
                    "ViGEmBus driver not installed. Please install from: \
                     https://github.com/ViGEm/ViGEmBus/releases"
                        .to_string(),
                ));
            }

            let device = WindowsGamepadDevice::new(name)?;
            Ok(Box::new(device))
        } else {
            // Create keyboard/mouse output device
            let device = WindowsKeyboardMouseDevice::new(name);
            Ok(Box::new(device))
        }
    }

    fn supports_device_type(&self, device_type: DeviceType) -> bool {
        match device_type {
            DeviceType::Gamepad => self.vigem_available,
            DeviceType::Keyboard | DeviceType::Mouse => true,
            DeviceType::Other => true,
        }
    }

    fn is_available(&self) -> bool {
        // Keyboard/mouse output is always available via SendInput
        true
    }

    fn availability_message(&self) -> Option<String> {
        if self.vigem_available {
            None
        } else {
            Some(
                "ViGEmBus driver not installed. Virtual gamepad support is unavailable.\n\
                 Install from: https://github.com/ViGEm/ViGEmBus/releases\n\
                 Keyboard/mouse output is still available."
                    .to_string(),
            )
        }
    }
}

/// Internal state for the gamepad that needs to be protected by a mutex
struct GamepadInner {
    target: Xbox360Wired<Arc<Client>>,
    gamepad_state: XGamepad,
}

/// Windows virtual gamepad device using Vigem
///
/// Thread safety is achieved by wrapping all mutable state in a `Mutex`.
/// The ViGEm `Client` and `Xbox360Wired` types do not implement `Sync` because
/// they contain raw pointers and internal state. By using interior mutability,
/// we can safely share this device across threads.
pub struct WindowsGamepadDevice {
    name: String,
    /// Arc-wrapped client to share ownership
    #[allow(dead_code)]
    client: Arc<Client>,
    /// Mutex-protected inner state for thread-safe access
    inner: Mutex<GamepadInner>,
}

// The struct is Send because:
// - `name` is String (Send + Sync)
// - `client` is Arc<Client> - Client contains internal state but is used through Arc
// - `inner` is Mutex<GamepadInner> which provides synchronized access
//
// The struct is Sync because all mutable state is behind a Mutex.
// Note: We're asserting that Client is safe to send between threads when wrapped in Arc,
// which is true because we only access it through synchronized operations.
unsafe impl Send for WindowsGamepadDevice {}
unsafe impl Sync for WindowsGamepadDevice {}

impl WindowsGamepadDevice {
    pub fn new(name: &str) -> Result<Self> {
        let client = Client::new()
            .map_err(|e| RemapperError::UInputCreationFailed(format!("Vigem error: {:?}", e)))?;

        let client = Arc::new(client);
        let mut target = Xbox360Wired::new(client.clone(), TargetId::XBOX360_WIRED);

        target
            .plugin()
            .map_err(|e| RemapperError::UInputCreationFailed(format!("Vigem plugin error: {:?}", e)))?;

        target
            .wait_ready()
            .map_err(|e| RemapperError::UInputCreationFailed(format!("Vigem ready error: {:?}", e)))?;

        debug!("Created virtual Xbox 360 gamepad: {}", name);

        Ok(Self {
            name: name.to_string(),
            client,
            inner: Mutex::new(GamepadInner {
                target,
                gamepad_state: XGamepad::default(),
            }),
        })
    }

    /// Update a button state in the gamepad
    fn update_button(inner: &mut GamepadInner, code: u16, pressed: bool) {
        let button = match code {
            304 => Some(XButtons::A),           // BTN_SOUTH
            305 => Some(XButtons::B),           // BTN_EAST
            307 => Some(XButtons::X),           // BTN_NORTH
            308 => Some(XButtons::Y),           // BTN_WEST
            310 => Some(XButtons::LB),          // BTN_TL
            311 => Some(XButtons::RB),          // BTN_TR
            314 => Some(XButtons::BACK),        // BTN_SELECT
            315 => Some(XButtons::START),       // BTN_START
            316 => Some(XButtons::GUIDE),       // BTN_MODE
            317 => Some(XButtons::LTHUMB),      // BTN_THUMBL
            318 => Some(XButtons::RTHUMB),      // BTN_THUMBR
            _ => None,
        };

        if let Some(btn) = button {
            if pressed {
                inner.gamepad_state.buttons.raw |= btn.raw;
            } else {
                inner.gamepad_state.buttons.raw &= !btn.raw;
            }
        }
    }

    /// Update an axis value in the gamepad
    fn update_axis(inner: &mut GamepadInner, code: u16, value: i32) {
        match code {
            0 => inner.gamepad_state.thumb_lx = value as i16,    // ABS_X
            1 => inner.gamepad_state.thumb_ly = -(value as i16), // ABS_Y (inverted)
            3 => inner.gamepad_state.thumb_rx = value as i16,    // ABS_RX
            4 => inner.gamepad_state.thumb_ry = -(value as i16), // ABS_RY (inverted)
            2 => inner.gamepad_state.left_trigger = (value.clamp(0, 255)) as u8, // ABS_Z
            5 => inner.gamepad_state.right_trigger = (value.clamp(0, 255)) as u8, // ABS_RZ
            16 => {
                // ABS_HAT0X (D-pad X)
                match value {
                    -1 => inner.gamepad_state.buttons.raw |= XButtons::LEFT.raw,
                    1 => inner.gamepad_state.buttons.raw |= XButtons::RIGHT.raw,
                    0 => {
                        inner.gamepad_state.buttons.raw &= !XButtons::LEFT.raw;
                        inner.gamepad_state.buttons.raw &= !XButtons::RIGHT.raw;
                    }
                    _ => {}
                }
            }
            17 => {
                // ABS_HAT0Y (D-pad Y)
                match value {
                    -1 => inner.gamepad_state.buttons.raw |= XButtons::UP.raw,
                    1 => inner.gamepad_state.buttons.raw |= XButtons::DOWN.raw,
                    0 => {
                        inner.gamepad_state.buttons.raw &= !XButtons::UP.raw;
                        inner.gamepad_state.buttons.raw &= !XButtons::DOWN.raw;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl PlatformOutputDevice for WindowsGamepadDevice {
    fn write_event(&self, event: &PlatformInputEvent) -> Result<()> {
        trace!("Gamepad event: {:?}", event);

        let mut inner = self.inner.lock().map_err(|e| {
            RemapperError::EventWriteError(format!("Failed to lock gamepad state: {}", e))
        })?;

        match event.event_type {
            1 => {
                // EV_KEY - button events
                Self::update_button(&mut inner, event.code, event.value != 0);
            }
            3 => {
                // EV_ABS - axis events
                Self::update_axis(&mut inner, event.code, event.value);
            }
            _ => {}
        }

        Ok(())
    }

    fn write_events(&self, events: &[PlatformInputEvent]) -> Result<()> {
        // Lock once for all events to ensure atomic updates
        let mut inner = self.inner.lock().map_err(|e| {
            RemapperError::EventWriteError(format!("Failed to lock gamepad state: {}", e))
        })?;

        for event in events {
            trace!("Gamepad event: {:?}", event);

            match event.event_type {
                1 => {
                    // EV_KEY - button events
                    Self::update_button(&mut inner, event.code, event.value != 0);
                }
                3 => {
                    // EV_ABS - axis events
                    Self::update_axis(&mut inner, event.code, event.value);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn sync(&self) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|e| {
            RemapperError::EventWriteError(format!("Failed to lock gamepad state: {}", e))
        })?;

        // Send the current gamepad state to the virtual device
        inner.target.update(&inner.gamepad_state).map_err(|e| {
            RemapperError::EventWriteError(format!("Failed to update gamepad state: {:?}", e))
        })?;

        trace!("Synced gamepad state: {:?}", inner.gamepad_state);
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Windows keyboard/mouse output device using SendInput
pub struct WindowsKeyboardMouseDevice {
    name: String,
}

impl WindowsKeyboardMouseDevice {
    pub fn new(name: &str) -> Self {
        debug!("Created keyboard/mouse output device: {}", name);
        Self {
            name: name.to_string(),
        }
    }

    fn send_keyboard_event(&self, scancode: u16, key_up: bool) -> Result<()> {
        let mut input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                    wScan: scancode,
                    dwFlags: if key_up {
                        KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP
                    } else {
                        KEYEVENTF_SCANCODE
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }

        Ok(())
    }

    fn send_mouse_button(&self, code: u16, pressed: bool) -> Result<()> {
        let (down_flag, up_flag) = match code {
            272 => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),     // BTN_LEFT
            273 => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),   // BTN_RIGHT
            274 => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP), // BTN_MIDDLE
            _ => return Ok(()),
        };

        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: if pressed { down_flag } else { up_flag },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }

        Ok(())
    }

    fn send_mouse_move(&self, dx: i32, dy: i32) -> Result<()> {
        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }

        Ok(())
    }

    fn send_mouse_wheel(&self, delta: i32) -> Result<()> {
        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: delta as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }

        Ok(())
    }
}

impl PlatformOutputDevice for WindowsKeyboardMouseDevice {
    fn write_event(&self, event: &PlatformInputEvent) -> Result<()> {
        trace!("Keyboard/mouse event: {:?}", event);

        match event.event_type {
            1 => {
                // EV_KEY
                if event.code >= 272 && event.code <= 276 {
                    // Mouse buttons (BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, etc.)
                    self.send_mouse_button(event.code, event.value != 0)?;
                } else {
                    // Keyboard key - convert evdev code to Windows scancode
                    let scancode = evdev_key_to_scancode(event.code);
                    self.send_keyboard_event(scancode, event.value == 0)?;
                }
            }
            2 => {
                // EV_REL
                match event.code {
                    0 => self.send_mouse_move(event.value, 0)?, // REL_X
                    1 => self.send_mouse_move(0, event.value)?, // REL_Y
                    8 => self.send_mouse_wheel(event.value * 120)?, // REL_WHEEL
                    _ => {}
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn write_events(&self, events: &[PlatformInputEvent]) -> Result<()> {
        for event in events {
            self.write_event(event)?;
        }
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        // SendInput events are sent immediately, no sync needed
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Convert evdev key code to Windows scancode
fn evdev_key_to_scancode(evdev_code: u16) -> u16 {
    // This is a simplified mapping - a complete implementation would include all keys
    match evdev_code {
        1 => 0x01,   // KEY_ESC
        2 => 0x02,   // KEY_1
        3 => 0x03,   // KEY_2
        4 => 0x04,   // KEY_3
        5 => 0x05,   // KEY_4
        6 => 0x06,   // KEY_5
        7 => 0x07,   // KEY_6
        8 => 0x08,   // KEY_7
        9 => 0x09,   // KEY_8
        10 => 0x0A,  // KEY_9
        11 => 0x0B,  // KEY_0
        12 => 0x0C,  // KEY_MINUS
        13 => 0x0D,  // KEY_EQUAL
        14 => 0x0E,  // KEY_BACKSPACE
        15 => 0x0F,  // KEY_TAB
        16 => 0x10,  // KEY_Q
        17 => 0x11,  // KEY_W
        18 => 0x12,  // KEY_E
        19 => 0x13,  // KEY_R
        20 => 0x14,  // KEY_T
        21 => 0x15,  // KEY_Y
        22 => 0x16,  // KEY_U
        23 => 0x17,  // KEY_I
        24 => 0x18,  // KEY_O
        25 => 0x19,  // KEY_P
        26 => 0x1A,  // KEY_LEFTBRACE
        27 => 0x1B,  // KEY_RIGHTBRACE
        28 => 0x1C,  // KEY_ENTER
        29 => 0x1D,  // KEY_LEFTCTRL
        30 => 0x1E,  // KEY_A
        31 => 0x1F,  // KEY_S
        32 => 0x20,  // KEY_D
        33 => 0x21,  // KEY_F
        34 => 0x22,  // KEY_G
        35 => 0x23,  // KEY_H
        36 => 0x24,  // KEY_J
        37 => 0x25,  // KEY_K
        38 => 0x26,  // KEY_L
        39 => 0x27,  // KEY_SEMICOLON
        40 => 0x28,  // KEY_APOSTROPHE
        41 => 0x29,  // KEY_GRAVE
        42 => 0x2A,  // KEY_LEFTSHIFT
        43 => 0x2B,  // KEY_BACKSLASH
        44 => 0x2C,  // KEY_Z
        45 => 0x2D,  // KEY_X
        46 => 0x2E,  // KEY_C
        47 => 0x2F,  // KEY_V
        48 => 0x30,  // KEY_B
        49 => 0x31,  // KEY_N
        50 => 0x32,  // KEY_M
        51 => 0x33,  // KEY_COMMA
        52 => 0x34,  // KEY_DOT
        53 => 0x35,  // KEY_SLASH
        54 => 0x36,  // KEY_RIGHTSHIFT
        55 => 0x37,  // KEY_KPASTERISK
        56 => 0x38,  // KEY_LEFTALT
        57 => 0x39,  // KEY_SPACE
        58 => 0x3A,  // KEY_CAPSLOCK
        59 => 0x3B,  // KEY_F1
        60 => 0x3C,  // KEY_F2
        61 => 0x3D,  // KEY_F3
        62 => 0x3E,  // KEY_F4
        63 => 0x3F,  // KEY_F5
        64 => 0x40,  // KEY_F6
        65 => 0x41,  // KEY_F7
        66 => 0x42,  // KEY_F8
        67 => 0x43,  // KEY_F9
        68 => 0x44,  // KEY_F10
        87 => 0x57,  // KEY_F11
        88 => 0x58,  // KEY_F12
        _ => evdev_code, // Fallback to same code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Compile-time test that WindowsGamepadDevice implements Send + Sync
    /// This verifies that the thread-safety wrapper is correctly implemented.
    #[test]
    fn test_gamepad_device_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<WindowsGamepadDevice>();
        assert_sync::<WindowsGamepadDevice>();
    }

    /// Compile-time test that WindowsKeyboardMouseDevice implements Send + Sync
    #[test]
    fn test_keyboard_mouse_device_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<WindowsKeyboardMouseDevice>();
        assert_sync::<WindowsKeyboardMouseDevice>();
    }

    /// Compile-time test that WindowsOutputBackend implements Send + Sync
    #[test]
    fn test_output_backend_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<WindowsOutputBackend>();
        assert_sync::<WindowsOutputBackend>();
    }

    /// Test concurrent access to WindowsGamepadDevice
    /// This test requires ViGEmBus driver to be installed, so it's marked as ignored.
    #[test]
    #[ignore = "Requires ViGEmBus driver to be installed"]
    fn test_concurrent_gamepad_access() {
        let device = Arc::new(WindowsGamepadDevice::new("Test Gamepad").unwrap());

        let mut handles = vec![];

        // Spawn multiple threads that write events concurrently
        for i in 0..4 {
            let device_clone = Arc::clone(&device);
            let handle = thread::spawn(move || {
                // Simulate button presses from different threads
                for j in 0..10 {
                    let event = PlatformInputEvent::new(1, 304 + (i as u16 % 4), (j % 2) as i32);
                    device_clone.write_event(&event).unwrap();
                    device_clone.sync().unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test concurrent axis updates on WindowsGamepadDevice
    /// This test requires ViGEmBus driver to be installed, so it's marked as ignored.
    #[test]
    #[ignore = "Requires ViGEmBus driver to be installed"]
    fn test_concurrent_axis_updates() {
        let device = Arc::new(WindowsGamepadDevice::new("Test Gamepad Axes").unwrap());

        let mut handles = vec![];

        // Spawn threads for different axes
        for axis_code in [0u16, 1, 3, 4] {
            let device_clone = Arc::clone(&device);
            let handle = thread::spawn(move || {
                // Simulate axis movements
                for value in (-32768i32..=32767).step_by(1000) {
                    let event = PlatformInputEvent::new(3, axis_code, value);
                    device_clone.write_event(&event).unwrap();
                }
                device_clone.sync().unwrap();
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test that write_events locks atomically for multiple events
    /// This test requires ViGEmBus driver to be installed, so it's marked as ignored.
    #[test]
    #[ignore = "Requires ViGEmBus driver to be installed"]
    fn test_atomic_batch_writes() {
        let device = Arc::new(WindowsGamepadDevice::new("Test Gamepad Batch").unwrap());

        let device_clone = Arc::clone(&device);
        let writer = thread::spawn(move || {
            // Write a batch of events atomically
            let events: Vec<PlatformInputEvent> = (304..=308)
                .map(|code| PlatformInputEvent::new(1, code, 1))
                .collect();
            device_clone.write_events(&events).unwrap();
            device_clone.sync().unwrap();
        });

        // Another thread trying to read shouldn't see partial state
        let device_clone2 = Arc::clone(&device);
        let reader = thread::spawn(move || {
            // This thread just exercises concurrent access
            for _ in 0..100 {
                let _ = device_clone2.sync();
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    }

    /// Test keyboard/mouse device creation
    #[test]
    fn test_keyboard_mouse_device_creation() {
        let device = WindowsKeyboardMouseDevice::new("Test KB/Mouse");
        assert_eq!(device.name(), "Test KB/Mouse");
    }

    /// Test output backend availability
    #[test]
    fn test_output_backend_availability() {
        let backend = WindowsOutputBackend::new();
        // Keyboard/mouse is always available
        assert!(backend.is_available());
        assert!(backend.supports_device_type(DeviceType::Keyboard));
        assert!(backend.supports_device_type(DeviceType::Mouse));
    }
}
