//! macOS virtual output device implementation using Core Graphics
//!
//! Note: CGEventSource is not Send/Sync, so we create new sources for each operation.
//! This is slightly less efficient but ensures thread safety.

use std::sync::Mutex;

use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use foreign_types_shared::ForeignType;
use tracing::{debug, trace, warn};

// FFI bindings for CGEventCreateScrollWheelEvent which is not exposed by core-graphics 0.24
mod ffi {
    use core_graphics::sys::{CGEventRef, CGEventSourceRef};

    /// CGScrollEventUnit specifies the unit of measurement for scroll events
    #[repr(u32)]
    #[derive(Debug, Clone, Copy)]
    pub enum CGScrollEventUnit {
        /// Scroll amount is specified in pixels
        Pixel = 0,
        /// Scroll amount is specified in lines (discrete scroll units)
        Line = 1,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        /// Creates a scroll wheel event.
        ///
        /// # Parameters
        /// - `source`: The event source (can be null)
        /// - `units`: Whether scroll is measured in lines or pixels
        /// - `wheel_count`: Number of scroll wheels (1 for vertical only, 2 for vertical+horizontal)
        /// - `wheel1`: Primary (vertical) scroll delta
        /// - `...`: Additional wheel deltas if wheel_count > 1
        pub fn CGEventCreateScrollWheelEvent(
            source: CGEventSourceRef,
            units: CGScrollEventUnit,
            wheel_count: u32,
            wheel1: i32,
            ...
        ) -> CGEventRef;
    }
}

use crate::core::error::{RemapperError, Result};
use crate::platform::traits::{
    DeviceCapabilities, DeviceType, OutputBackend, PlatformInputEvent, PlatformOutputDevice,
};

/// macOS output backend using Core Graphics
pub struct MacOSOutputBackend;

impl MacOSOutputBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOSOutputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputBackend for MacOSOutputBackend {
    fn create_device(
        &self,
        name: &str,
        capabilities: &DeviceCapabilities,
    ) -> Result<Box<dyn PlatformOutputDevice>> {
        // Check if this is a gamepad based on capabilities
        let has_gamepad_buttons = capabilities.keys.iter().any(|k| *k >= 304 && *k <= 318);
        let has_abs_axes = !capabilities.abs_axes.is_empty();

        if has_gamepad_buttons && has_abs_axes {
            // Virtual gamepad output is not supported on macOS without DriverKit
            return Err(RemapperError::NotSupported(
                "Virtual gamepad output is not yet supported on macOS. \
                 This requires DriverKit approval from Apple. \
                 Keyboard/mouse remapping is available."
                    .to_string(),
            ));
        }

        let device = MacOSOutputDevice::new(name)?;
        Ok(Box::new(device))
    }

    fn supports_device_type(&self, device_type: DeviceType) -> bool {
        match device_type {
            DeviceType::Keyboard | DeviceType::Mouse => true,
            DeviceType::Gamepad => false, // Not supported without DriverKit
            DeviceType::Other => true,
        }
    }

    fn is_available(&self) -> bool {
        // CGEventPost is always available, but may require accessibility permissions
        true
    }

    fn availability_message(&self) -> Option<String> {
        Some(
            "Virtual gamepad output requires DriverKit approval from Apple (not yet available).\n\
             Keyboard/mouse output requires Accessibility permissions in System Preferences.\n\
             Go to: System Preferences > Security & Privacy > Privacy > Accessibility"
                .to_string(),
        )
    }
}

/// macOS keyboard/mouse output device using Core Graphics
///
/// Note: We store the name but create CGEventSource on-demand for thread safety.
pub struct MacOSOutputDevice {
    name: String,
    // Use a mutex to provide interior mutability for mouse position tracking
    mouse_position: Mutex<(f64, f64)>,
}

// Manually implement Send and Sync since we're not storing CGEventSource
unsafe impl Send for MacOSOutputDevice {}
unsafe impl Sync for MacOSOutputDevice {}

impl MacOSOutputDevice {
    pub fn new(name: &str) -> Result<Self> {
        // Verify we can create an event source
        let _ = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| RemapperError::UInputCreationFailed("Failed to create event source".to_string()))?;

        debug!("Created macOS keyboard/mouse output device: {}", name);

        Ok(Self {
            name: name.to_string(),
            mouse_position: Mutex::new((0.0, 0.0)),
        })
    }

    fn get_event_source(&self) -> Result<CGEventSource> {
        CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| RemapperError::EventWriteError("Failed to create event source".to_string()))
    }

    fn post_keyboard_event(&self, keycode: u16, key_down: bool) -> Result<()> {
        let source = self.get_event_source()?;

        let event = CGEvent::new_keyboard_event(source, keycode, key_down)
            .map_err(|_| RemapperError::EventWriteError("Failed to create keyboard event".to_string()))?;

        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn post_mouse_button(&self, button: CGMouseButton, pressed: bool) -> Result<()> {
        let source = self.get_event_source()?;

        let event_type = match (button, pressed) {
            (CGMouseButton::Left, true) => CGEventType::LeftMouseDown,
            (CGMouseButton::Left, false) => CGEventType::LeftMouseUp,
            (CGMouseButton::Right, true) => CGEventType::RightMouseDown,
            (CGMouseButton::Right, false) => CGEventType::RightMouseUp,
            (CGMouseButton::Center, true) => CGEventType::OtherMouseDown,
            (CGMouseButton::Center, false) => CGEventType::OtherMouseUp,
        };

        let pos = self.mouse_position.lock().unwrap();
        let point = core_graphics::geometry::CGPoint::new(pos.0, pos.1);
        drop(pos);

        let event = CGEvent::new_mouse_event(source, event_type, point, button)
            .map_err(|_| RemapperError::EventWriteError("Failed to create mouse event".to_string()))?;

        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn post_mouse_move(&self, dx: i32, dy: i32) -> Result<()> {
        let source = self.get_event_source()?;

        let mut pos = self.mouse_position.lock().unwrap();
        pos.0 += dx as f64;
        pos.1 += dy as f64;
        let point = core_graphics::geometry::CGPoint::new(pos.0, pos.1);
        drop(pos);

        let event = CGEvent::new_mouse_event(
            source,
            CGEventType::MouseMoved,
            point,
            CGMouseButton::Left, // Not used for move events
        )
        .map_err(|_| RemapperError::EventWriteError("Failed to create mouse move event".to_string()))?;

        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn post_scroll(&self, delta_y: i32) -> Result<()> {
        self.post_scroll_wheel(delta_y, 0)
    }

    fn post_scroll_horizontal(&self, delta_x: i32) -> Result<()> {
        self.post_scroll_wheel(0, delta_x)
    }

    fn post_scroll_wheel(&self, delta_y: i32, delta_x: i32) -> Result<()> {
        let source = self.get_event_source()?;

        // Safety: CGEventCreateScrollWheelEvent is a well-documented CoreGraphics function.
        // We use Line units for discrete scroll events (matching typical mouse wheel behavior).
        // The source reference is valid for the duration of this call.
        let event_ref = unsafe {
            ffi::CGEventCreateScrollWheelEvent(
                source.as_ptr(),
                ffi::CGScrollEventUnit::Line,
                2, // wheel_count: 2 for both vertical and horizontal
                delta_y,
                delta_x,
            )
        };

        if event_ref.is_null() {
            return Err(RemapperError::EventWriteError(
                "Failed to create scroll wheel event".to_string(),
            ));
        }

        // Wrap in CGEvent to get automatic memory management and use the post method
        // Safety: event_ref is a valid, non-null CGEventRef that we just created
        let event = unsafe { CGEvent::from_ptr(event_ref) };
        event.post(CGEventTapLocation::HID);

        trace!("Posted scroll event: delta_y={}, delta_x={}", delta_y, delta_x);
        Ok(())
    }
}

impl PlatformOutputDevice for MacOSOutputDevice {
    fn write_event(&self, event: &PlatformInputEvent) -> Result<()> {
        trace!("macOS output event: {:?}", event);

        match event.event_type {
            1 => {
                // EV_KEY
                if event.code >= 272 && event.code <= 276 {
                    // Mouse buttons
                    let button = match event.code {
                        272 => CGMouseButton::Left,
                        273 => CGMouseButton::Right,
                        274 => CGMouseButton::Center,
                        _ => return Ok(()),
                    };
                    self.post_mouse_button(button, event.value != 0)?;
                } else {
                    // Keyboard key
                    let keycode = evdev_key_to_macos_keycode(event.code);
                    self.post_keyboard_event(keycode, event.value != 0)?;
                }
            }
            2 => {
                // EV_REL
                match event.code {
                    0 => self.post_mouse_move(event.value, 0)?,       // REL_X
                    1 => self.post_mouse_move(0, event.value)?,       // REL_Y
                    6 => self.post_scroll_horizontal(event.value)?,   // REL_HWHEEL
                    8 => self.post_scroll(event.value)?,              // REL_WHEEL
                    _ => {}
                }
            }
            3 => {
                // EV_ABS - gamepad events, not supported for output on macOS
                warn!("Gamepad output events are not supported on macOS");
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
        // CGEventPost events are sent immediately, no sync needed
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Convert evdev key code to macOS virtual keycode
fn evdev_key_to_macos_keycode(evdev_code: u16) -> u16 {
    // macOS uses different keycodes than evdev
    // This is a simplified mapping for common keys
    match evdev_code {
        1 => 0x35,   // KEY_ESC -> kVK_Escape
        2 => 0x12,   // KEY_1 -> kVK_ANSI_1
        3 => 0x13,   // KEY_2 -> kVK_ANSI_2
        4 => 0x14,   // KEY_3 -> kVK_ANSI_3
        5 => 0x15,   // KEY_4 -> kVK_ANSI_4
        6 => 0x17,   // KEY_5 -> kVK_ANSI_5
        7 => 0x16,   // KEY_6 -> kVK_ANSI_6
        8 => 0x1A,   // KEY_7 -> kVK_ANSI_7
        9 => 0x1C,   // KEY_8 -> kVK_ANSI_8
        10 => 0x19,  // KEY_9 -> kVK_ANSI_9
        11 => 0x1D,  // KEY_0 -> kVK_ANSI_0
        12 => 0x1B,  // KEY_MINUS -> kVK_ANSI_Minus
        13 => 0x18,  // KEY_EQUAL -> kVK_ANSI_Equal
        14 => 0x33,  // KEY_BACKSPACE -> kVK_Delete
        15 => 0x30,  // KEY_TAB -> kVK_Tab
        16 => 0x0C,  // KEY_Q -> kVK_ANSI_Q
        17 => 0x0D,  // KEY_W -> kVK_ANSI_W
        18 => 0x0E,  // KEY_E -> kVK_ANSI_E
        19 => 0x0F,  // KEY_R -> kVK_ANSI_R
        20 => 0x11,  // KEY_T -> kVK_ANSI_T
        21 => 0x10,  // KEY_Y -> kVK_ANSI_Y
        22 => 0x20,  // KEY_U -> kVK_ANSI_U
        23 => 0x22,  // KEY_I -> kVK_ANSI_I
        24 => 0x1F,  // KEY_O -> kVK_ANSI_O
        25 => 0x23,  // KEY_P -> kVK_ANSI_P
        26 => 0x21,  // KEY_LEFTBRACE -> kVK_ANSI_LeftBracket
        27 => 0x1E,  // KEY_RIGHTBRACE -> kVK_ANSI_RightBracket
        28 => 0x24,  // KEY_ENTER -> kVK_Return
        29 => 0x3B,  // KEY_LEFTCTRL -> kVK_Control
        30 => 0x00,  // KEY_A -> kVK_ANSI_A
        31 => 0x01,  // KEY_S -> kVK_ANSI_S
        32 => 0x02,  // KEY_D -> kVK_ANSI_D
        33 => 0x03,  // KEY_F -> kVK_ANSI_F
        34 => 0x05,  // KEY_G -> kVK_ANSI_G
        35 => 0x04,  // KEY_H -> kVK_ANSI_H
        36 => 0x26,  // KEY_J -> kVK_ANSI_J
        37 => 0x28,  // KEY_K -> kVK_ANSI_K
        38 => 0x25,  // KEY_L -> kVK_ANSI_L
        39 => 0x29,  // KEY_SEMICOLON -> kVK_ANSI_Semicolon
        40 => 0x27,  // KEY_APOSTROPHE -> kVK_ANSI_Quote
        41 => 0x32,  // KEY_GRAVE -> kVK_ANSI_Grave
        42 => 0x38,  // KEY_LEFTSHIFT -> kVK_Shift
        43 => 0x2A,  // KEY_BACKSLASH -> kVK_ANSI_Backslash
        44 => 0x06,  // KEY_Z -> kVK_ANSI_Z
        45 => 0x07,  // KEY_X -> kVK_ANSI_X
        46 => 0x08,  // KEY_C -> kVK_ANSI_C
        47 => 0x09,  // KEY_V -> kVK_ANSI_V
        48 => 0x0B,  // KEY_B -> kVK_ANSI_B
        49 => 0x2D,  // KEY_N -> kVK_ANSI_N
        50 => 0x2E,  // KEY_M -> kVK_ANSI_M
        51 => 0x2B,  // KEY_COMMA -> kVK_ANSI_Comma
        52 => 0x2F,  // KEY_DOT -> kVK_ANSI_Period
        53 => 0x2C,  // KEY_SLASH -> kVK_ANSI_Slash
        54 => 0x3C,  // KEY_RIGHTSHIFT -> kVK_RightShift
        56 => 0x3A,  // KEY_LEFTALT -> kVK_Option
        57 => 0x31,  // KEY_SPACE -> kVK_Space
        58 => 0x39,  // KEY_CAPSLOCK -> kVK_CapsLock
        59 => 0x7A,  // KEY_F1 -> kVK_F1
        60 => 0x78,  // KEY_F2 -> kVK_F2
        61 => 0x63,  // KEY_F3 -> kVK_F3
        62 => 0x76,  // KEY_F4 -> kVK_F4
        63 => 0x60,  // KEY_F5 -> kVK_F5
        64 => 0x61,  // KEY_F6 -> kVK_F6
        65 => 0x62,  // KEY_F7 -> kVK_F7
        66 => 0x64,  // KEY_F8 -> kVK_F8
        67 => 0x65,  // KEY_F9 -> kVK_F9
        68 => 0x6D,  // KEY_F10 -> kVK_F10
        87 => 0x67,  // KEY_F11 -> kVK_F11
        88 => 0x6F,  // KEY_F12 -> kVK_F12
        103 => 0x7E, // KEY_UP -> kVK_UpArrow
        105 => 0x7B, // KEY_LEFT -> kVK_LeftArrow
        106 => 0x7C, // KEY_RIGHT -> kVK_RightArrow
        108 => 0x7D, // KEY_DOWN -> kVK_DownArrow
        125 => 0x37, // KEY_LEFTMETA -> kVK_Command
        _ => evdev_code, // Fallback
    }
}
