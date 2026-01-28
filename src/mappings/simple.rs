//! Simple 1:1 key mapping

use async_trait::async_trait;
use tracing::trace;

use crate::core::error::{RemapperError, Result};
use crate::core::events::{parse_key_code_numeric, parse_abs_code_numeric, parse_rel_code_numeric, EventCode, EventType, InputEvent};

use super::MappingHandler;

/// Handler for simple 1:1 mappings
pub struct SimpleHandler {
    /// Source event code
    from: EventCode,
    /// Source event code (numeric)
    from_code: u16,
    /// Target event code
    to: EventCode,
    /// Target event code (numeric)
    to_code: u16,
}

impl SimpleHandler {
    /// Create a new simple mapping handler
    pub fn new(from: EventCode, to: EventCode) -> Result<Self> {
        // Parse source code
        let from_code = Self::parse_code(&from)?;
        let to_code = Self::parse_code(&to)?;

        Ok(Self {
            from,
            from_code,
            to,
            to_code,
        })
    }

    /// Parse an event code to numeric value
    fn parse_code(code: &EventCode) -> Result<u16> {
        match code.event_type {
            EventType::Key => {
                if let Some(key_code) = parse_key_code_numeric(&code.code) {
                    Ok(key_code)
                } else {
                    // Try parsing as a number
                    code.code
                        .parse::<u16>()
                        .map_err(|_| RemapperError::InvalidMapping(
                            format!("Unknown key code: {}", code.code)
                        ))
                }
            }
            EventType::Abs => {
                if let Some(axis_code) = parse_abs_code_numeric(&code.code) {
                    Ok(axis_code)
                } else {
                    code.code
                        .parse::<u16>()
                        .map_err(|_| RemapperError::InvalidMapping(
                            format!("Unknown abs code: {}", code.code)
                        ))
                }
            }
            EventType::Rel => {
                if let Some(axis_code) = parse_rel_code_numeric(&code.code) {
                    Ok(axis_code)
                } else {
                    code.code
                        .parse::<u16>()
                        .map_err(|_| RemapperError::InvalidMapping(
                            format!("Unknown rel code: {}", code.code)
                        ))
                }
            }
            _ => {
                code.code
                    .parse::<u16>()
                    .map_err(|_| RemapperError::InvalidMapping(
                        format!("Cannot parse event code: {}", code)
                    ))
            }
        }
    }
}

#[async_trait]
impl MappingHandler for SimpleHandler {
    fn handles(&self, event: &InputEvent) -> bool {
        event.event_type == self.from.event_type && event.code == self.from_code
    }

    async fn process(&mut self, event: InputEvent) -> Vec<InputEvent> {
        trace!(
            "Simple remap: {} -> {}",
            self.from.code,
            self.to.code
        );

        // Create remapped event with same value but different code
        vec![InputEvent::new(
            self.to.event_type,
            self.to_code,
            event.value,
        )]
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn description(&self) -> String {
        format!("{} -> {}", self.from, self.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_handler() {
        let mut handler = SimpleHandler::new(
            EventCode::key("BTN_A"),
            EventCode::key("BTN_B"),
        ).unwrap();

        // Create a button press event for BTN_A (code 304)
        let event = InputEvent::new(EventType::Key, 304, 1);

        assert!(handler.handles(&event));

        let output = handler.process(event).await;
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].code, 305); // BTN_B
        assert_eq!(output[0].value, 1);
    }

    #[tokio::test]
    async fn test_simple_handler_passthrough() {
        let handler = SimpleHandler::new(
            EventCode::key("BTN_A"),
            EventCode::key("BTN_B"),
        ).unwrap();

        // Create an event for a different button
        let event = InputEvent::new(EventType::Key, 305, 1);

        assert!(!handler.handles(&event));
    }
}
