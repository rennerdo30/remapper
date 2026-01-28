//! Windows virtual output device implementation using Vigem and SendInput

use std::sync::Arc;

use tracing::{debug, trace, warn};
use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
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

/// Windows virtual gamepad device using Vigem
pub struct WindowsGamepadDevice {
    name: String,
    client: Client,
    target: Xbox360Wired<Arc<Client>>,
    gamepad_state: XGamepad,
}

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
            client: Client::new().unwrap(), // Keep a reference to prevent drop
            target,
            gamepad_state: XGamepad::default(),
        })
    }

    fn update_button(&mut self, code: u16, pressed: bool) {
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
                self.gamepad_state.buttons.raw |= btn.raw;
            } else {
                self.gamepad_state.buttons.raw &= !btn.raw;
            }
        }
    }

    fn update_axis(&mut self, code: u16, value: i32) {
        match code {
            0 => self.gamepad_state.thumb_lx = value as i16,    // ABS_X
            1 => self.gamepad_state.thumb_ly = -(value as i16), // ABS_Y (inverted)
            3 => self.gamepad_state.thumb_rx = value as i16,    // ABS_RX
            4 => self.gamepad_state.thumb_ry = -(value as i16), // ABS_RY (inverted)
            2 => self.gamepad_state.left_trigger = (value.clamp(0, 255)) as u8, // ABS_Z
            5 => self.gamepad_state.right_trigger = (value.clamp(0, 255)) as u8, // ABS_RZ
            16 => {
                // ABS_HAT0X (D-pad X)
                match value {
                    -1 => self.gamepad_state.buttons.raw |= XButtons::LEFT.raw,
                    1 => self.gamepad_state.buttons.raw |= XButtons::RIGHT.raw,
                    0 => {
                        self.gamepad_state.buttons.raw &= !XButtons::LEFT.raw;
                        self.gamepad_state.buttons.raw &= !XButtons::RIGHT.raw;
                    }
                    _ => {}
                }
            }
            17 => {
                // ABS_HAT0Y (D-pad Y)
                match value {
                    -1 => self.gamepad_state.buttons.raw |= XButtons::UP.raw,
                    1 => self.gamepad_state.buttons.raw |= XButtons::DOWN.raw,
                    0 => {
                        self.gamepad_state.buttons.raw &= !XButtons::UP.raw;
                        self.gamepad_state.buttons.raw &= !XButtons::DOWN.raw;
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
        // We need mutable access, but the trait requires &self
        // This is a limitation we work around by updating state in sync()
        trace!("Gamepad event: {:?}", event);
        Ok(())
    }

    fn write_events(&self, events: &[PlatformInputEvent]) -> Result<()> {
        for event in events {
            self.write_event(event)?;
        }
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        // Note: This requires mutable self, which is a trait design issue
        // For now, we'd need interior mutability or trait redesign
        // This is a placeholder - real implementation would use Mutex
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
