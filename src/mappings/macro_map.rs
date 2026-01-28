//! Macro mapping - trigger a sequence of events

use async_trait::async_trait;
use tracing::{debug, trace};

use crate::config::MacroStep;
use crate::core::error::{RemapperError, Result};
use crate::core::events::{parse_key_code_numeric, EventCode, EventType, InputEvent};

use super::MappingHandler;

/// Handler for macro mappings
pub struct MacroHandler {
    /// Trigger event code
    trigger: EventCode,
    /// Trigger event code (numeric)
    trigger_code: u16,
    /// Sequence of steps to execute
    sequence: Vec<MacroStep>,
    /// Compiled sequence (code, value pairs and delays)
    compiled: Vec<CompiledStep>,
    /// Whether the trigger is currently held
    trigger_held: bool,
}

/// Compiled macro step for faster execution
enum CompiledStep {
    /// Key event with code and value
    Key { code: u16, value: i32 },
    /// Delay in milliseconds
    Delay(u32),
}

impl MacroHandler {
    /// Create a new macro handler
    pub fn new(trigger: EventCode, sequence: Vec<MacroStep>) -> Result<Self> {
        // Parse trigger code
        let trigger_code = Self::parse_key_code(&trigger)?;

        // Compile sequence
        let compiled = Self::compile_sequence(&sequence)?;

        Ok(Self {
            trigger,
            trigger_code,
            sequence,
            compiled,
            trigger_held: false,
        })
    }

    /// Parse a key event code to numeric value
    fn parse_key_code(code: &EventCode) -> Result<u16> {
        if code.event_type != EventType::Key {
            return Err(RemapperError::InvalidMapping(
                "Macro trigger must be a key event".into(),
            ));
        }

        if let Some(key_code) = parse_key_code_numeric(&code.code) {
            Ok(key_code)
        } else {
            code.code
                .parse::<u16>()
                .map_err(|_| RemapperError::InvalidMapping(format!("Unknown key: {}", code.code)))
        }
    }

    /// Compile macro steps
    fn compile_sequence(sequence: &[MacroStep]) -> Result<Vec<CompiledStep>> {
        let mut compiled = Vec::new();

        for step in sequence {
            match step {
                MacroStep::Key { code, value } => {
                    let key_code = if let Some(parsed_code) = parse_key_code_numeric(code) {
                        parsed_code
                    } else {
                        code.parse::<u16>().map_err(|_| {
                            RemapperError::InvalidMapping(format!("Unknown key: {}", code))
                        })?
                    };
                    compiled.push(CompiledStep::Key {
                        code: key_code,
                        value: *value,
                    });
                }
                MacroStep::Delay { delay_ms } => {
                    compiled.push(CompiledStep::Delay(*delay_ms));
                }
            }
        }

        Ok(compiled)
    }

    /// Execute the macro sequence
    async fn execute_sequence(&self) -> Vec<InputEvent> {
        let mut events = Vec::new();

        for step in &self.compiled {
            match step {
                CompiledStep::Key { code, value } => {
                    events.push(InputEvent::new(EventType::Key, *code, *value));
                    // Add sync after each key event
                    events.push(InputEvent::sync());
                }
                CompiledStep::Delay(ms) => {
                    // Execute delay
                    tokio::time::sleep(std::time::Duration::from_millis(*ms as u64)).await;
                }
            }
        }

        events
    }
}

#[async_trait]
impl MappingHandler for MacroHandler {
    fn handles(&self, event: &InputEvent) -> bool {
        event.event_type == EventType::Key && event.code == self.trigger_code
    }

    async fn process(&mut self, event: InputEvent) -> Vec<InputEvent> {
        if event.value == 1 && !self.trigger_held {
            // Key press - execute macro
            self.trigger_held = true;
            debug!("Executing macro for trigger: {}", self.trigger.code);
            self.execute_sequence().await
        } else if event.value == 0 {
            // Key release
            self.trigger_held = false;
            trace!("Macro trigger released: {}", self.trigger.code);
            Vec::new()
        } else {
            // Key repeat - ignore
            Vec::new()
        }
    }

    fn reset(&mut self) {
        self.trigger_held = false;
    }

    fn description(&self) -> String {
        format!(
            "{} -> [macro with {} steps]",
            self.trigger,
            self.sequence.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_macro_handler() {
        let sequence = vec![
            MacroStep::press("KEY_LEFTALT"),
            MacroStep::press("KEY_TAB"),
            MacroStep::delay(50),
            MacroStep::release("KEY_TAB"),
            MacroStep::release("KEY_LEFTALT"),
        ];

        let mut handler =
            MacroHandler::new(EventCode::key("BTN_SELECT"), sequence).unwrap();

        // Trigger press
        let event = InputEvent::new(EventType::Key, 314, 1); // BTN_SELECT
        assert!(handler.handles(&event));

        let output = handler.process(event).await;
        // Should have key events + syncs: (press alt, sync, press tab, sync, release tab, sync, release alt, sync)
        // = 8 events, but delays don't produce events
        assert!(!output.is_empty());
    }
}
