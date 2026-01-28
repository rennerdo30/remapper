//! Custom theme for the Remapper GUI

use iced::Color;

/// Custom theme colors
pub struct RemapperTheme;

impl RemapperTheme {
    /// Primary accent color
    pub const PRIMARY: Color = Color::from_rgb(0.2, 0.6, 1.0);

    /// Success/running color
    pub const SUCCESS: Color = Color::from_rgb(0.2, 0.8, 0.4);

    /// Warning color
    pub const WARNING: Color = Color::from_rgb(1.0, 0.8, 0.2);

    /// Danger/error color
    pub const DANGER: Color = Color::from_rgb(0.9, 0.3, 0.3);

    /// Background color
    pub const BACKGROUND: Color = Color::from_rgb(0.12, 0.12, 0.14);

    /// Surface color (cards, panels)
    pub const SURFACE: Color = Color::from_rgb(0.18, 0.18, 0.20);

    /// Border color
    pub const BORDER: Color = Color::from_rgb(0.25, 0.25, 0.28);

    /// Text primary
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.9, 0.9, 0.92);

    /// Text secondary/muted
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.6, 0.6, 0.65);

    /// Gamepad badge color
    pub const BADGE_GAMEPAD: Color = Color::from_rgb(0.3, 0.8, 0.4);

    /// Keyboard badge color
    pub const BADGE_KEYBOARD: Color = Color::from_rgb(0.3, 0.6, 1.0);

    /// Mouse badge color
    pub const BADGE_MOUSE: Color = Color::from_rgb(1.0, 0.7, 0.2);
}

/// Status indicator colors
pub mod status {
    use super::*;

    /// Running status
    pub const RUNNING: Color = RemapperTheme::SUCCESS;

    /// Stopped status
    pub const STOPPED: Color = Color::from_rgb(0.4, 0.4, 0.45);

    /// Starting/stopping status
    pub const TRANSITIONING: Color = RemapperTheme::WARNING;

    /// Error status
    pub const ERROR: Color = RemapperTheme::DANGER;
}
