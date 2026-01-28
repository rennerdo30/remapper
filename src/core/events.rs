//! Event types and conversions

use serde::{Deserialize, Serialize};
use std::fmt;

/// Event type categories (EV_KEY, EV_ABS, EV_REL, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    /// Synchronization events
    #[serde(rename = "EV_SYN")]
    Syn,
    /// Key/button events
    #[serde(rename = "EV_KEY")]
    Key,
    /// Relative axis events (mouse movement)
    #[serde(rename = "EV_REL")]
    Rel,
    /// Absolute axis events (joystick, touchscreen)
    #[serde(rename = "EV_ABS")]
    Abs,
    /// Miscellaneous events
    #[serde(rename = "EV_MSC")]
    Msc,
    /// Switch events
    #[serde(rename = "EV_SW")]
    Sw,
    /// LED events
    #[serde(rename = "EV_LED")]
    Led,
    /// Sound events
    #[serde(rename = "EV_SND")]
    Snd,
    /// Repeat events
    #[serde(rename = "EV_REP")]
    Rep,
    /// Force feedback events
    #[serde(rename = "EV_FF")]
    Ff,
    /// Power events
    #[serde(rename = "EV_PWR")]
    Pwr,
    /// Force feedback status
    #[serde(rename = "EV_FF_STATUS")]
    FfStatus,
}

impl EventType {
    /// Convert from evdev event type (Linux only)
    #[cfg(target_os = "linux")]
    pub fn from_evdev(ev_type: evdev::EventType) -> Option<Self> {
        match ev_type {
            evdev::EventType::SYNCHRONIZATION => Some(EventType::Syn),
            evdev::EventType::KEY => Some(EventType::Key),
            evdev::EventType::RELATIVE => Some(EventType::Rel),
            evdev::EventType::ABSOLUTE => Some(EventType::Abs),
            evdev::EventType::MISC => Some(EventType::Msc),
            evdev::EventType::SWITCH => Some(EventType::Sw),
            evdev::EventType::LED => Some(EventType::Led),
            evdev::EventType::SOUND => Some(EventType::Snd),
            evdev::EventType::REPEAT => Some(EventType::Rep),
            evdev::EventType::FORCEFEEDBACK => Some(EventType::Ff),
            evdev::EventType::POWER => Some(EventType::Pwr),
            evdev::EventType::FORCEFEEDBACKSTATUS => Some(EventType::FfStatus),
            _ => None,
        }
    }

    /// Convert to evdev event type (Linux only)
    #[cfg(target_os = "linux")]
    pub fn to_evdev(self) -> evdev::EventType {
        match self {
            EventType::Syn => evdev::EventType::SYNCHRONIZATION,
            EventType::Key => evdev::EventType::KEY,
            EventType::Rel => evdev::EventType::RELATIVE,
            EventType::Abs => evdev::EventType::ABSOLUTE,
            EventType::Msc => evdev::EventType::MISC,
            EventType::Sw => evdev::EventType::SWITCH,
            EventType::Led => evdev::EventType::LED,
            EventType::Snd => evdev::EventType::SOUND,
            EventType::Rep => evdev::EventType::REPEAT,
            EventType::Ff => evdev::EventType::FORCEFEEDBACK,
            EventType::Pwr => evdev::EventType::POWER,
            EventType::FfStatus => evdev::EventType::FORCEFEEDBACKSTATUS,
        }
    }

    /// Convert from numeric event type code
    pub fn from_code(code: u16) -> Option<Self> {
        match code {
            0 => Some(EventType::Syn),
            1 => Some(EventType::Key),
            2 => Some(EventType::Rel),
            3 => Some(EventType::Abs),
            4 => Some(EventType::Msc),
            5 => Some(EventType::Sw),
            17 => Some(EventType::Led),
            18 => Some(EventType::Snd),
            20 => Some(EventType::Rep),
            21 => Some(EventType::Ff),
            22 => Some(EventType::Pwr),
            23 => Some(EventType::FfStatus),
            _ => None,
        }
    }

    /// Convert to numeric event type code
    pub fn to_code(self) -> u16 {
        match self {
            EventType::Syn => 0,
            EventType::Key => 1,
            EventType::Rel => 2,
            EventType::Abs => 3,
            EventType::Msc => 4,
            EventType::Sw => 5,
            EventType::Led => 17,
            EventType::Snd => 18,
            EventType::Rep => 20,
            EventType::Ff => 21,
            EventType::Pwr => 22,
            EventType::FfStatus => 23,
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::Syn => write!(f, "EV_SYN"),
            EventType::Key => write!(f, "EV_KEY"),
            EventType::Rel => write!(f, "EV_REL"),
            EventType::Abs => write!(f, "EV_ABS"),
            EventType::Msc => write!(f, "EV_MSC"),
            EventType::Sw => write!(f, "EV_SW"),
            EventType::Led => write!(f, "EV_LED"),
            EventType::Snd => write!(f, "EV_SND"),
            EventType::Rep => write!(f, "EV_REP"),
            EventType::Ff => write!(f, "EV_FF"),
            EventType::Pwr => write!(f, "EV_PWR"),
            EventType::FfStatus => write!(f, "EV_FF_STATUS"),
        }
    }
}

/// Event code identifying specific key/axis/button
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventCode {
    /// Event type (EV_KEY, EV_ABS, etc.)
    #[serde(rename = "type")]
    pub event_type: EventType,
    /// Event code name (e.g., "BTN_A", "KEY_ESC", "ABS_X")
    pub code: String,
}

impl EventCode {
    /// Create a new event code
    pub fn new(event_type: EventType, code: impl Into<String>) -> Self {
        Self {
            event_type,
            code: code.into(),
        }
    }

    /// Create a key event code
    pub fn key(code: impl Into<String>) -> Self {
        Self::new(EventType::Key, code)
    }

    /// Create an absolute axis event code
    pub fn abs(code: impl Into<String>) -> Self {
        Self::new(EventType::Abs, code)
    }

    /// Create a relative axis event code
    pub fn rel(code: impl Into<String>) -> Self {
        Self::new(EventType::Rel, code)
    }

    /// Parse code string to evdev Key (Linux only)
    #[cfg(target_os = "linux")]
    pub fn to_evdev_key(&self) -> Option<evdev::Key> {
        parse_key_code(&self.code)
    }

    /// Parse code string to evdev AbsoluteAxisType (Linux only)
    #[cfg(target_os = "linux")]
    pub fn to_evdev_abs(&self) -> Option<evdev::AbsoluteAxisType> {
        parse_abs_code(&self.code)
    }

    /// Parse code string to evdev RelativeAxisType (Linux only)
    #[cfg(target_os = "linux")]
    pub fn to_evdev_rel(&self) -> Option<evdev::RelativeAxisType> {
        parse_rel_code(&self.code)
    }

    /// Parse code string to numeric key code
    pub fn to_key_code(&self) -> Option<u16> {
        parse_key_code_numeric(&self.code)
    }
}

impl fmt::Display for EventCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.event_type, self.code)
    }
}

/// Input event with type, code, and value
#[derive(Debug, Clone)]
pub struct InputEvent {
    /// Event type
    pub event_type: EventType,
    /// Event code (raw numeric value)
    pub code: u16,
    /// Event value
    pub value: i32,
    /// Timestamp seconds
    pub time_sec: i64,
    /// Timestamp microseconds
    pub time_usec: i64,
}

impl InputEvent {
    /// Create a new input event
    pub fn new(event_type: EventType, code: u16, value: i32) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            event_type,
            code,
            value,
            time_sec: now.as_secs() as i64,
            time_usec: now.subsec_micros() as i64,
        }
    }

    /// Create a sync event
    pub fn sync() -> Self {
        Self::new(EventType::Syn, 0, 0)
    }

    /// Create a key press event
    pub fn key_press(code: u16) -> Self {
        Self::new(EventType::Key, code, 1)
    }

    /// Create a key release event
    pub fn key_release(code: u16) -> Self {
        Self::new(EventType::Key, code, 0)
    }

    /// Check if this is a key press
    pub fn is_key_press(&self) -> bool {
        self.event_type == EventType::Key && self.value == 1
    }

    /// Check if this is a key release
    pub fn is_key_release(&self) -> bool {
        self.event_type == EventType::Key && self.value == 0
    }

    /// Check if this is a sync event
    pub fn is_sync(&self) -> bool {
        self.event_type == EventType::Syn
    }

    /// Convert from evdev InputEvent (Linux only)
    #[cfg(target_os = "linux")]
    pub fn from_evdev(event: &evdev::InputEvent) -> Option<Self> {
        let event_type = EventType::from_evdev(event.event_type())?;
        Some(Self {
            event_type,
            code: event.code(),
            value: event.value(),
            time_sec: event.timestamp().tv_sec,
            time_usec: event.timestamp().tv_usec,
        })
    }

    /// Convert to evdev InputEvent
    #[cfg(target_os = "linux")]
    pub fn to_evdev(&self) -> evdev::InputEvent {
        evdev::InputEvent::new(self.event_type.to_evdev(), self.code, self.value)
    }

    /// Convert from platform input event
    pub fn from_platform(event: &crate::platform::PlatformInputEvent) -> Self {
        let event_type = match event.event_type {
            0 => EventType::Syn,
            1 => EventType::Key,
            2 => EventType::Rel,
            3 => EventType::Abs,
            4 => EventType::Msc,
            5 => EventType::Sw,
            17 => EventType::Led,
            18 => EventType::Snd,
            20 => EventType::Rep,
            21 => EventType::Ff,
            22 => EventType::Pwr,
            23 => EventType::FfStatus,
            _ => EventType::Syn, // Default to sync for unknown
        };

        Self {
            event_type,
            code: event.code,
            value: event.value,
            time_sec: (event.timestamp_us / 1_000_000) as i64,
            time_usec: (event.timestamp_us % 1_000_000) as i64,
        }
    }

    /// Convert to platform input event
    pub fn to_platform(&self) -> crate::platform::PlatformInputEvent {
        let event_type = match self.event_type {
            EventType::Syn => 0,
            EventType::Key => 1,
            EventType::Rel => 2,
            EventType::Abs => 3,
            EventType::Msc => 4,
            EventType::Sw => 5,
            EventType::Led => 17,
            EventType::Snd => 18,
            EventType::Rep => 20,
            EventType::Ff => 21,
            EventType::Pwr => 22,
            EventType::FfStatus => 23,
        };

        crate::platform::PlatformInputEvent {
            event_type,
            code: self.code,
            value: self.value,
            timestamp_us: (self.time_sec as u64 * 1_000_000) + (self.time_usec as u64),
        }
    }
}

impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} code={} value={}",
            self.event_type, self.code, self.value
        )
    }
}

/// Parse a key code string to numeric code (cross-platform)
pub fn parse_key_code_numeric(code: &str) -> Option<u16> {
    match code {
        // Function keys
        "KEY_ESC" => Some(1),
        "KEY_1" => Some(2),
        "KEY_2" => Some(3),
        "KEY_3" => Some(4),
        "KEY_4" => Some(5),
        "KEY_5" => Some(6),
        "KEY_6" => Some(7),
        "KEY_7" => Some(8),
        "KEY_8" => Some(9),
        "KEY_9" => Some(10),
        "KEY_0" => Some(11),
        "KEY_MINUS" => Some(12),
        "KEY_EQUAL" => Some(13),
        "KEY_BACKSPACE" => Some(14),
        "KEY_TAB" => Some(15),
        "KEY_Q" => Some(16),
        "KEY_W" => Some(17),
        "KEY_E" => Some(18),
        "KEY_R" => Some(19),
        "KEY_T" => Some(20),
        "KEY_Y" => Some(21),
        "KEY_U" => Some(22),
        "KEY_I" => Some(23),
        "KEY_O" => Some(24),
        "KEY_P" => Some(25),
        "KEY_LEFTBRACE" => Some(26),
        "KEY_RIGHTBRACE" => Some(27),
        "KEY_ENTER" => Some(28),
        "KEY_LEFTCTRL" => Some(29),
        "KEY_A" => Some(30),
        "KEY_S" => Some(31),
        "KEY_D" => Some(32),
        "KEY_F" => Some(33),
        "KEY_G" => Some(34),
        "KEY_H" => Some(35),
        "KEY_J" => Some(36),
        "KEY_K" => Some(37),
        "KEY_L" => Some(38),
        "KEY_SEMICOLON" => Some(39),
        "KEY_APOSTROPHE" => Some(40),
        "KEY_GRAVE" => Some(41),
        "KEY_LEFTSHIFT" => Some(42),
        "KEY_BACKSLASH" => Some(43),
        "KEY_Z" => Some(44),
        "KEY_X" => Some(45),
        "KEY_C" => Some(46),
        "KEY_V" => Some(47),
        "KEY_B" => Some(48),
        "KEY_N" => Some(49),
        "KEY_M" => Some(50),
        "KEY_COMMA" => Some(51),
        "KEY_DOT" => Some(52),
        "KEY_SLASH" => Some(53),
        "KEY_RIGHTSHIFT" => Some(54),
        "KEY_KPASTERISK" => Some(55),
        "KEY_LEFTALT" => Some(56),
        "KEY_SPACE" => Some(57),
        "KEY_CAPSLOCK" => Some(58),
        "KEY_F1" => Some(59),
        "KEY_F2" => Some(60),
        "KEY_F3" => Some(61),
        "KEY_F4" => Some(62),
        "KEY_F5" => Some(63),
        "KEY_F6" => Some(64),
        "KEY_F7" => Some(65),
        "KEY_F8" => Some(66),
        "KEY_F9" => Some(67),
        "KEY_F10" => Some(68),
        "KEY_F11" => Some(87),
        "KEY_F12" => Some(88),
        "KEY_RIGHTCTRL" => Some(97),
        "KEY_RIGHTALT" => Some(100),
        "KEY_HOME" => Some(102),
        "KEY_UP" => Some(103),
        "KEY_PAGEUP" => Some(104),
        "KEY_LEFT" => Some(105),
        "KEY_RIGHT" => Some(106),
        "KEY_END" => Some(107),
        "KEY_DOWN" => Some(108),
        "KEY_PAGEDOWN" => Some(109),
        "KEY_INSERT" => Some(110),
        "KEY_DELETE" => Some(111),
        "KEY_LEFTMETA" => Some(125),
        "KEY_RIGHTMETA" => Some(126),

        // Gamepad buttons
        "BTN_A" | "BTN_SOUTH" => Some(304),
        "BTN_B" | "BTN_EAST" => Some(305),
        "BTN_C" => Some(306),
        "BTN_X" | "BTN_NORTH" => Some(307),
        "BTN_Y" | "BTN_WEST" => Some(308),
        "BTN_Z" => Some(309),
        "BTN_TL" => Some(310),
        "BTN_TR" => Some(311),
        "BTN_TL2" => Some(312),
        "BTN_TR2" => Some(313),
        "BTN_SELECT" => Some(314),
        "BTN_START" => Some(315),
        "BTN_MODE" => Some(316),
        "BTN_THUMBL" => Some(317),
        "BTN_THUMBR" => Some(318),
        "BTN_DPAD_UP" => Some(544),
        "BTN_DPAD_DOWN" => Some(545),
        "BTN_DPAD_LEFT" => Some(546),
        "BTN_DPAD_RIGHT" => Some(547),

        // Mouse buttons
        "BTN_LEFT" => Some(272),
        "BTN_RIGHT" => Some(273),
        "BTN_MIDDLE" => Some(274),
        "BTN_SIDE" => Some(275),
        "BTN_EXTRA" => Some(276),
        "BTN_FORWARD" => Some(277),
        "BTN_BACK" => Some(278),

        _ => None,
    }
}

/// Parse an absolute axis code string to numeric code (cross-platform)
pub fn parse_abs_code_numeric(code: &str) -> Option<u16> {
    match code {
        "ABS_X" => Some(0),
        "ABS_Y" => Some(1),
        "ABS_Z" => Some(2),
        "ABS_RX" => Some(3),
        "ABS_RY" => Some(4),
        "ABS_RZ" => Some(5),
        "ABS_THROTTLE" => Some(6),
        "ABS_RUDDER" => Some(7),
        "ABS_WHEEL" => Some(8),
        "ABS_GAS" => Some(9),
        "ABS_BRAKE" => Some(10),
        "ABS_HAT0X" => Some(16),
        "ABS_HAT0Y" => Some(17),
        "ABS_HAT1X" => Some(18),
        "ABS_HAT1Y" => Some(19),
        "ABS_HAT2X" => Some(20),
        "ABS_HAT2Y" => Some(21),
        "ABS_HAT3X" => Some(22),
        "ABS_HAT3Y" => Some(23),
        "ABS_PRESSURE" => Some(24),
        "ABS_DISTANCE" => Some(25),
        "ABS_TILT_X" => Some(26),
        "ABS_TILT_Y" => Some(27),
        "ABS_MISC" => Some(40),
        _ => None,
    }
}

/// Parse a relative axis code string to numeric code (cross-platform)
pub fn parse_rel_code_numeric(code: &str) -> Option<u16> {
    match code {
        "REL_X" => Some(0),
        "REL_Y" => Some(1),
        "REL_Z" => Some(2),
        "REL_RX" => Some(3),
        "REL_RY" => Some(4),
        "REL_RZ" => Some(5),
        "REL_HWHEEL" => Some(6),
        "REL_DIAL" => Some(7),
        "REL_WHEEL" => Some(8),
        "REL_MISC" => Some(9),
        _ => None,
    }
}

/// Get human-readable name for a key code (cross-platform)
pub fn key_code_to_name(code: u16) -> String {
    match code {
        1 => "KEY_ESC".to_string(),
        2 => "KEY_1".to_string(),
        3 => "KEY_2".to_string(),
        4 => "KEY_3".to_string(),
        5 => "KEY_4".to_string(),
        6 => "KEY_5".to_string(),
        7 => "KEY_6".to_string(),
        8 => "KEY_7".to_string(),
        9 => "KEY_8".to_string(),
        10 => "KEY_9".to_string(),
        11 => "KEY_0".to_string(),
        12 => "KEY_MINUS".to_string(),
        13 => "KEY_EQUAL".to_string(),
        14 => "KEY_BACKSPACE".to_string(),
        15 => "KEY_TAB".to_string(),
        16 => "KEY_Q".to_string(),
        17 => "KEY_W".to_string(),
        18 => "KEY_E".to_string(),
        19 => "KEY_R".to_string(),
        20 => "KEY_T".to_string(),
        21 => "KEY_Y".to_string(),
        22 => "KEY_U".to_string(),
        23 => "KEY_I".to_string(),
        24 => "KEY_O".to_string(),
        25 => "KEY_P".to_string(),
        26 => "KEY_LEFTBRACE".to_string(),
        27 => "KEY_RIGHTBRACE".to_string(),
        28 => "KEY_ENTER".to_string(),
        29 => "KEY_LEFTCTRL".to_string(),
        30 => "KEY_A".to_string(),
        31 => "KEY_S".to_string(),
        32 => "KEY_D".to_string(),
        33 => "KEY_F".to_string(),
        34 => "KEY_G".to_string(),
        35 => "KEY_H".to_string(),
        36 => "KEY_J".to_string(),
        37 => "KEY_K".to_string(),
        38 => "KEY_L".to_string(),
        39 => "KEY_SEMICOLON".to_string(),
        40 => "KEY_APOSTROPHE".to_string(),
        41 => "KEY_GRAVE".to_string(),
        42 => "KEY_LEFTSHIFT".to_string(),
        43 => "KEY_BACKSLASH".to_string(),
        44 => "KEY_Z".to_string(),
        45 => "KEY_X".to_string(),
        46 => "KEY_C".to_string(),
        47 => "KEY_V".to_string(),
        48 => "KEY_B".to_string(),
        49 => "KEY_N".to_string(),
        50 => "KEY_M".to_string(),
        51 => "KEY_COMMA".to_string(),
        52 => "KEY_DOT".to_string(),
        53 => "KEY_SLASH".to_string(),
        54 => "KEY_RIGHTSHIFT".to_string(),
        55 => "KEY_KPASTERISK".to_string(),
        56 => "KEY_LEFTALT".to_string(),
        57 => "KEY_SPACE".to_string(),
        58 => "KEY_CAPSLOCK".to_string(),
        59 => "KEY_F1".to_string(),
        60 => "KEY_F2".to_string(),
        61 => "KEY_F3".to_string(),
        62 => "KEY_F4".to_string(),
        63 => "KEY_F5".to_string(),
        64 => "KEY_F6".to_string(),
        65 => "KEY_F7".to_string(),
        66 => "KEY_F8".to_string(),
        67 => "KEY_F9".to_string(),
        68 => "KEY_F10".to_string(),
        87 => "KEY_F11".to_string(),
        88 => "KEY_F12".to_string(),
        97 => "KEY_RIGHTCTRL".to_string(),
        100 => "KEY_RIGHTALT".to_string(),
        102 => "KEY_HOME".to_string(),
        103 => "KEY_UP".to_string(),
        104 => "KEY_PAGEUP".to_string(),
        105 => "KEY_LEFT".to_string(),
        106 => "KEY_RIGHT".to_string(),
        107 => "KEY_END".to_string(),
        108 => "KEY_DOWN".to_string(),
        109 => "KEY_PAGEDOWN".to_string(),
        110 => "KEY_INSERT".to_string(),
        111 => "KEY_DELETE".to_string(),
        125 => "KEY_LEFTMETA".to_string(),
        126 => "KEY_RIGHTMETA".to_string(),
        // Mouse buttons
        272 => "BTN_LEFT".to_string(),
        273 => "BTN_RIGHT".to_string(),
        274 => "BTN_MIDDLE".to_string(),
        275 => "BTN_SIDE".to_string(),
        276 => "BTN_EXTRA".to_string(),
        // Gamepad buttons
        304 => "BTN_SOUTH".to_string(),
        305 => "BTN_EAST".to_string(),
        306 => "BTN_C".to_string(),
        307 => "BTN_NORTH".to_string(),
        308 => "BTN_WEST".to_string(),
        309 => "BTN_Z".to_string(),
        310 => "BTN_TL".to_string(),
        311 => "BTN_TR".to_string(),
        312 => "BTN_TL2".to_string(),
        313 => "BTN_TR2".to_string(),
        314 => "BTN_SELECT".to_string(),
        315 => "BTN_START".to_string(),
        316 => "BTN_MODE".to_string(),
        317 => "BTN_THUMBL".to_string(),
        318 => "BTN_THUMBR".to_string(),
        _ => format!("KEY_{}", code),
    }
}

/// Get human-readable name for an absolute axis code (cross-platform)
pub fn abs_code_to_name(code: u16) -> String {
    match code {
        0 => "ABS_X".to_string(),
        1 => "ABS_Y".to_string(),
        2 => "ABS_Z".to_string(),
        3 => "ABS_RX".to_string(),
        4 => "ABS_RY".to_string(),
        5 => "ABS_RZ".to_string(),
        6 => "ABS_THROTTLE".to_string(),
        7 => "ABS_RUDDER".to_string(),
        8 => "ABS_WHEEL".to_string(),
        9 => "ABS_GAS".to_string(),
        10 => "ABS_BRAKE".to_string(),
        16 => "ABS_HAT0X".to_string(),
        17 => "ABS_HAT0Y".to_string(),
        18 => "ABS_HAT1X".to_string(),
        19 => "ABS_HAT1Y".to_string(),
        20 => "ABS_HAT2X".to_string(),
        21 => "ABS_HAT2Y".to_string(),
        22 => "ABS_HAT3X".to_string(),
        23 => "ABS_HAT3Y".to_string(),
        24 => "ABS_PRESSURE".to_string(),
        25 => "ABS_DISTANCE".to_string(),
        26 => "ABS_TILT_X".to_string(),
        27 => "ABS_TILT_Y".to_string(),
        40 => "ABS_MISC".to_string(),
        _ => format!("ABS_{}", code),
    }
}

/// Get human-readable name for a relative axis code (cross-platform)
pub fn rel_code_to_name(code: u16) -> String {
    match code {
        0 => "REL_X".to_string(),
        1 => "REL_Y".to_string(),
        2 => "REL_Z".to_string(),
        3 => "REL_RX".to_string(),
        4 => "REL_RY".to_string(),
        5 => "REL_RZ".to_string(),
        6 => "REL_HWHEEL".to_string(),
        7 => "REL_DIAL".to_string(),
        8 => "REL_WHEEL".to_string(),
        9 => "REL_MISC".to_string(),
        _ => format!("REL_{}", code),
    }
}

// Linux-specific evdev parsing functions
#[cfg(target_os = "linux")]
mod evdev_parsing {
    use super::*;

    /// Parse a key code string like "KEY_A" or "BTN_A" to evdev Key
    pub fn parse_key_code(code: &str) -> Option<evdev::Key> {
        parse_key_code_numeric(code).map(evdev::Key::new)
    }

    /// Parse an absolute axis code string to evdev AbsoluteAxisType
    pub fn parse_abs_code(code: &str) -> Option<evdev::AbsoluteAxisType> {
        parse_abs_code_numeric(code).map(evdev::AbsoluteAxisType)
    }

    /// Parse a relative axis code string to evdev RelativeAxisType
    pub fn parse_rel_code(code: &str) -> Option<evdev::RelativeAxisType> {
        parse_rel_code_numeric(code).map(evdev::RelativeAxisType)
    }

    /// Get the string name for a key code
    pub fn key_code_name(key: evdev::Key) -> String {
        format!("{:?}", key)
    }

    /// Get the string name for an absolute axis code
    pub fn abs_code_name(axis: evdev::AbsoluteAxisType) -> String {
        format!("{:?}", axis)
    }

    /// Get the string name for a relative axis code
    pub fn rel_code_name(axis: evdev::RelativeAxisType) -> String {
        format!("{:?}", axis)
    }
}

#[cfg(target_os = "linux")]
pub use evdev_parsing::*;
