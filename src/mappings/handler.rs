//! Mapping handler trait

use async_trait::async_trait;

use crate::core::events::InputEvent;

/// Trait for mapping handlers
///
/// Each mapping type (simple, macro, conditional, combo) implements this trait
/// to process input events and produce output events.
#[async_trait]
pub trait MappingHandler: Send + Sync {
    /// Check if this handler handles the given event
    fn handles(&self, event: &InputEvent) -> bool;

    /// Process an input event and return output events
    ///
    /// May return:
    /// - Empty vec: event consumed, no output
    /// - Single event: simple remap
    /// - Multiple events: macro expansion
    async fn process(&mut self, event: InputEvent) -> Vec<InputEvent>;

    /// Reset handler state
    ///
    /// Called when engine stops or on error recovery
    fn reset(&mut self);

    /// Get a description of this mapping
    fn description(&self) -> String;
}
