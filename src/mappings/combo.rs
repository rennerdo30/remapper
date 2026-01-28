//! Combo mapping - multiple keys pressed together

use async_trait::async_trait;
use std::collections::HashSet;
use tracing::{debug, trace};

use crate::core::error::{RemapperError, Result};
use crate::core::events::{parse_key_code_numeric, EventCode, EventType, InputEvent};

use super::MappingHandler;

/// Handler for combo (chord) mappings
pub struct ComboHandler {
    /// Required keys for the combo
    keys: Vec<EventCode>,
    /// Required key codes (numeric)
    key_codes: HashSet<u16>,
    /// First key code (for ordering check)
    first_key: u16,
    /// Output event code
    output: EventCode,
    /// Output code (numeric)
    output_code: u16,
    /// Whether key order matters
    order_sensitive: bool,
    /// Currently held keys
    held_keys: HashSet<u16>,
    /// Order in which keys were pressed (if order sensitive)
    press_order: Vec<u16>,
    /// Whether combo has been triggered
    combo_triggered: bool,
}

impl ComboHandler {
    /// Create a new combo handler
    pub fn new(keys: Vec<EventCode>, output: EventCode, order_sensitive: bool) -> Result<Self> {
        if keys.len() < 2 {
            return Err(RemapperError::InvalidMapping(
                "Combo must have at least 2 keys".into(),
            ));
        }

        let mut key_codes = HashSet::new();
        let mut first_key = 0u16;

        for (i, key) in keys.iter().enumerate() {
            let code = Self::parse_key_code(key)?;
            if i == 0 {
                first_key = code;
            }
            key_codes.insert(code);
        }

        let output_code = Self::parse_key_code(&output)?;

        Ok(Self {
            keys,
            key_codes,
            first_key,
            output,
            output_code,
            order_sensitive,
            held_keys: HashSet::new(),
            press_order: Vec::new(),
            combo_triggered: false,
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

    /// Check if the combo is complete
    fn is_combo_complete(&self) -> bool {
        if self.held_keys.len() != self.key_codes.len() {
            return false;
        }

        if self.order_sensitive {
            // Check order matches
            let expected_order: Vec<u16> = self.keys
                .iter()
                .filter_map(|k| parse_key_code_numeric(&k.code))
                .collect();
            self.press_order == expected_order
        } else {
            // Just check all keys are held
            self.held_keys == self.key_codes
        }
    }
}

#[async_trait]
impl MappingHandler for ComboHandler {
    fn handles(&self, event: &InputEvent) -> bool {
        event.event_type == EventType::Key && self.key_codes.contains(&event.code)
    }

    async fn process(&mut self, event: InputEvent) -> Vec<InputEvent> {
        match event.value {
            1 => {
                // Key press
                trace!("Combo: key {} pressed", event.code);
                self.held_keys.insert(event.code);
                if self.order_sensitive {
                    self.press_order.push(event.code);
                }

                // Check if combo is complete
                if self.is_combo_complete() && !self.combo_triggered {
                    self.combo_triggered = true;
                    debug!("Combo triggered: {:?} -> {}", self.keys, self.output);
                    return vec![
                        InputEvent::new(EventType::Key, self.output_code, 1),
                        InputEvent::sync(),
                    ];
                }

                Vec::new()
            }
            0 => {
                // Key release
                trace!("Combo: key {} released", event.code);
                self.held_keys.remove(&event.code);

                if self.combo_triggered {
                    // Release output when any combo key is released
                    self.combo_triggered = false;
                    self.press_order.clear();
                    debug!("Combo released");
                    return vec![
                        InputEvent::new(EventType::Key, self.output_code, 0),
                        InputEvent::sync(),
                    ];
                }

                // Remove from order tracking
                if self.order_sensitive {
                    self.press_order.retain(|&k| k != event.code);
                }

                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.held_keys.clear();
        self.press_order.clear();
        self.combo_triggered = false;
    }

    fn description(&self) -> String {
        let keys_str: Vec<String> = self.keys.iter().map(|k| k.code.clone()).collect();
        format!(
            "{} -> {} (order_sensitive={})",
            keys_str.join("+"),
            self.output,
            self.order_sensitive
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_combo_handler() {
        let mut handler = ComboHandler::new(
            vec![EventCode::key("BTN_TL"), EventCode::key("BTN_TR")],
            EventCode::key("KEY_ESC"),
            false,
        )
        .unwrap();

        // Press first key
        let press1 = InputEvent::new(EventType::Key, 310, 1); // BTN_TL
        assert!(handler.handles(&press1));
        let output = handler.process(press1).await;
        assert!(output.is_empty()); // Combo not complete

        // Press second key
        let press2 = InputEvent::new(EventType::Key, 311, 1); // BTN_TR
        let output = handler.process(press2).await;
        assert!(!output.is_empty()); // Combo complete
        assert_eq!(output[0].code, 1); // KEY_ESC
        assert_eq!(output[0].value, 1);

        // Release one key
        let release1 = InputEvent::new(EventType::Key, 310, 0);
        let output = handler.process(release1).await;
        assert!(!output.is_empty()); // Release output
        assert_eq!(output[0].code, 1); // KEY_ESC
        assert_eq!(output[0].value, 0);
    }

    #[tokio::test]
    async fn test_combo_order_sensitive() {
        let mut handler = ComboHandler::new(
            vec![EventCode::key("BTN_TL"), EventCode::key("BTN_TR")],
            EventCode::key("KEY_ESC"),
            true, // Order sensitive
        )
        .unwrap();

        // Press in wrong order (TR then TL)
        let press1 = InputEvent::new(EventType::Key, 311, 1); // BTN_TR
        handler.process(press1).await;

        let press2 = InputEvent::new(EventType::Key, 310, 1); // BTN_TL
        let output = handler.process(press2).await;
        assert!(output.is_empty()); // Wrong order, combo not triggered

        // Reset and try correct order
        handler.reset();

        let press1 = InputEvent::new(EventType::Key, 310, 1); // BTN_TL
        handler.process(press1).await;

        let press2 = InputEvent::new(EventType::Key, 311, 1); // BTN_TR
        let output = handler.process(press2).await;
        assert!(!output.is_empty()); // Correct order, combo triggered
    }
}
