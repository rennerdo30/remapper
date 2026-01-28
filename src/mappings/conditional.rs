//! Conditional mapping - tap vs hold behavior

use async_trait::async_trait;
use std::time::{Duration, Instant};
use tracing::{debug, trace};

use crate::core::error::{RemapperError, Result};
use crate::core::events::{parse_key_code_numeric, EventCode, EventType, InputEvent};

use super::MappingHandler;

/// Handler for conditional (tap/hold) mappings
pub struct ConditionalHandler {
    /// Trigger event code
    trigger: EventCode,
    /// Trigger event code (numeric)
    trigger_code: u16,
    /// Event to send on tap (short press)
    tap: Option<EventCode>,
    /// Tap code (numeric)
    tap_code: Option<u16>,
    /// Event to send on hold (long press)
    hold: Option<EventCode>,
    /// Hold code (numeric)
    hold_code: Option<u16>,
    /// Threshold in milliseconds to distinguish tap from hold
    threshold: Duration,
    /// When the trigger was pressed
    press_time: Option<Instant>,
    /// Whether we've already sent the hold event
    hold_sent: bool,
}

impl ConditionalHandler {
    /// Create a new conditional handler
    pub fn new(
        trigger: EventCode,
        tap: Option<EventCode>,
        hold: Option<EventCode>,
        threshold_ms: u32,
    ) -> Result<Self> {
        if tap.is_none() && hold.is_none() {
            return Err(RemapperError::InvalidMapping(
                "Conditional mapping must have at least tap or hold defined".into(),
            ));
        }

        let trigger_code = Self::parse_key_code(&trigger)?;
        let tap_code = tap.as_ref().map(|c| Self::parse_key_code(c)).transpose()?;
        let hold_code = hold.as_ref().map(|c| Self::parse_key_code(c)).transpose()?;

        Ok(Self {
            trigger,
            trigger_code,
            tap,
            tap_code,
            hold,
            hold_code,
            threshold: Duration::from_millis(threshold_ms as u64),
            press_time: None,
            hold_sent: false,
        })
    }

    /// Parse a key event code to numeric value
    fn parse_key_code(code: &EventCode) -> Result<u16> {
        if let Some(key_code) = parse_key_code_numeric(&code.code) {
            Ok(key_code)
        } else {
            code.code
                .parse::<u16>()
                .map_err(|_| RemapperError::InvalidMapping(format!("Unknown key: {}", code.code)))
        }
    }
}

#[async_trait]
impl MappingHandler for ConditionalHandler {
    fn handles(&self, event: &InputEvent) -> bool {
        event.event_type == EventType::Key && event.code == self.trigger_code
    }

    async fn process(&mut self, event: InputEvent) -> Vec<InputEvent> {
        match event.value {
            1 => {
                // Key press - start timer
                self.press_time = Some(Instant::now());
                self.hold_sent = false;
                trace!("Conditional: trigger pressed, starting timer");
                Vec::new()
            }
            2 => {
                // Key repeat - check if we should send hold
                if !self.hold_sent {
                    if let Some(press_time) = self.press_time {
                        if press_time.elapsed() >= self.threshold {
                            if let Some(hold_code) = self.hold_code {
                                self.hold_sent = true;
                                debug!("Conditional: threshold exceeded, sending hold");
                                return vec![
                                    InputEvent::new(EventType::Key, hold_code, 1),
                                    InputEvent::sync(),
                                ];
                            }
                        }
                    }
                }
                Vec::new()
            }
            0 => {
                // Key release
                if let Some(press_time) = self.press_time.take() {
                    let elapsed = press_time.elapsed();

                    if self.hold_sent {
                        // Release the hold key
                        if let Some(hold_code) = self.hold_code {
                            debug!("Conditional: releasing hold key");
                            return vec![
                                InputEvent::new(EventType::Key, hold_code, 0),
                                InputEvent::sync(),
                            ];
                        }
                    } else if elapsed < self.threshold {
                        // Tap - send tap key press and release
                        if let Some(tap_code) = self.tap_code {
                            debug!("Conditional: tap detected ({}ms)", elapsed.as_millis());
                            return vec![
                                InputEvent::new(EventType::Key, tap_code, 1),
                                InputEvent::sync(),
                                InputEvent::new(EventType::Key, tap_code, 0),
                                InputEvent::sync(),
                            ];
                        }
                    } else {
                        // Hold but hold_sent is false - send hold press and release
                        if let Some(hold_code) = self.hold_code {
                            debug!("Conditional: hold detected ({}ms)", elapsed.as_millis());
                            return vec![
                                InputEvent::new(EventType::Key, hold_code, 1),
                                InputEvent::sync(),
                                InputEvent::new(EventType::Key, hold_code, 0),
                                InputEvent::sync(),
                            ];
                        }
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.press_time = None;
        self.hold_sent = false;
    }

    fn description(&self) -> String {
        let tap_str = self
            .tap
            .as_ref()
            .map(|c| format!("tap={}", c))
            .unwrap_or_default();
        let hold_str = self
            .hold
            .as_ref()
            .map(|c| format!("hold={}", c))
            .unwrap_or_default();
        format!(
            "{} -> [{} {} threshold={}ms]",
            self.trigger,
            tap_str,
            hold_str,
            self.threshold.as_millis()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conditional_tap() {
        let mut handler = ConditionalHandler::new(
            EventCode::key("BTN_START"),
            Some(EventCode::key("KEY_ESC")),
            Some(EventCode::key("KEY_LEFTMETA")),
            300,
        )
        .unwrap();

        // Press
        let press = InputEvent::new(EventType::Key, 315, 1);
        assert!(handler.handles(&press));
        let output = handler.process(press).await;
        assert!(output.is_empty()); // No output on press

        // Quick release (tap)
        let release = InputEvent::new(EventType::Key, 315, 0);
        let output = handler.process(release).await;

        // Should get tap key press and release
        assert_eq!(output.len(), 4); // press, sync, release, sync
        assert_eq!(output[0].code, 1); // KEY_ESC
        assert_eq!(output[0].value, 1);
        assert_eq!(output[2].code, 1); // KEY_ESC
        assert_eq!(output[2].value, 0);
    }

    #[tokio::test]
    async fn test_conditional_hold() {
        let mut handler = ConditionalHandler::new(
            EventCode::key("BTN_START"),
            Some(EventCode::key("KEY_ESC")),
            Some(EventCode::key("KEY_LEFTMETA")),
            50, // Short threshold for testing
        )
        .unwrap();

        // Press
        let press = InputEvent::new(EventType::Key, 315, 1);
        handler.process(press).await;

        // Wait past threshold
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Repeat (simulating held key)
        let repeat = InputEvent::new(EventType::Key, 315, 2);
        let output = handler.process(repeat).await;

        // Should get hold key press
        assert!(!output.is_empty());
        assert_eq!(output[0].code, 125); // KEY_LEFTMETA
        assert_eq!(output[0].value, 1);
    }
}
