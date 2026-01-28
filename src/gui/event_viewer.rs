//! Real-time event viewer component

use iced::widget::{button, column, container, pick_list, row, scrollable, text, Column};
use iced::{Element, Length};
use std::collections::VecDeque;

use crate::core::events::{EventType, InputEvent};
use crate::devices::DeviceInfo;

/// Maximum number of events to keep in history
const MAX_EVENTS: usize = 100;

/// Event viewer component for debugging
pub struct EventViewer {
    /// Available devices
    devices: Vec<DeviceInfo>,
    /// Selected device index
    selected_device: Option<usize>,
    /// Whether currently capturing
    capturing: bool,
    /// Event history
    events: VecDeque<EventEntry>,
    /// Filter by event type
    filter: EventFilter,
    /// Whether to pause capture
    paused: bool,
}

/// Entry in the event log
#[derive(Debug, Clone)]
pub struct EventEntry {
    pub timestamp: String,
    pub event_type: String,
    pub code: u16,
    pub code_name: String,
    pub value: i32,
    pub value_str: String,
}

/// Event type filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventFilter {
    All,
    Keys,
    Axes,
    Buttons,
}

impl std::fmt::Display for EventFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventFilter::All => write!(f, "All Events"),
            EventFilter::Keys => write!(f, "Keys Only"),
            EventFilter::Axes => write!(f, "Axes Only"),
            EventFilter::Buttons => write!(f, "Buttons Only"),
        }
    }
}

/// Event viewer messages
#[derive(Debug, Clone)]
pub enum EventViewerMessage {
    /// Device selected
    DeviceSelected(usize),
    /// Start capturing
    StartCapture,
    /// Stop capturing
    StopCapture,
    /// Toggle pause
    TogglePause,
    /// Clear events
    ClearEvents,
    /// Filter changed
    FilterChanged(EventFilter),
    /// New event received
    EventReceived(EventEntry),
    /// Refresh device list
    RefreshDevices,
}

impl EventViewer {
    /// Create a new event viewer
    pub fn new(devices: Vec<DeviceInfo>) -> Self {
        Self {
            devices,
            selected_device: None,
            capturing: false,
            events: VecDeque::new(),
            filter: EventFilter::All,
            paused: false,
        }
    }

    /// Add an event to the log
    pub fn add_event(&mut self, event: InputEvent) {
        if self.paused {
            return;
        }

        // Check filter
        let should_add = match self.filter {
            EventFilter::All => true,
            EventFilter::Keys => event.event_type == EventType::Key,
            EventFilter::Axes => {
                event.event_type == EventType::Abs || event.event_type == EventType::Rel
            }
            EventFilter::Buttons => {
                event.event_type == EventType::Key && event.code >= 256
            }
        };

        if !should_add {
            return;
        }

        // Skip sync events
        if event.is_sync() {
            return;
        }

        let entry = EventEntry {
            timestamp: format!("{}.{:06}", event.time_sec, event.time_usec),
            event_type: format!("{}", event.event_type),
            code: event.code,
            code_name: get_code_name(&event),
            value: event.value,
            value_str: get_value_string(&event),
        };

        self.events.push_front(entry);

        // Limit history size
        while self.events.len() > MAX_EVENTS {
            self.events.pop_back();
        }
    }

    /// Handle message
    pub fn update(&mut self, message: EventViewerMessage) {
        match message {
            EventViewerMessage::DeviceSelected(idx) => {
                self.selected_device = Some(idx);
            }
            EventViewerMessage::StartCapture => {
                self.capturing = true;
            }
            EventViewerMessage::StopCapture => {
                self.capturing = false;
            }
            EventViewerMessage::TogglePause => {
                self.paused = !self.paused;
            }
            EventViewerMessage::ClearEvents => {
                self.events.clear();
            }
            EventViewerMessage::FilterChanged(filter) => {
                self.filter = filter;
            }
            EventViewerMessage::EventReceived(entry) => {
                self.events.push_front(entry);
                while self.events.len() > MAX_EVENTS {
                    self.events.pop_back();
                }
            }
            EventViewerMessage::RefreshDevices => {
                // Parent should refresh
            }
        }
    }

    /// Render the event viewer
    pub fn view(&self) -> Element<'_, EventViewerMessage> {
        let device_names: Vec<String> = self
            .devices
            .iter()
            .map(|d| format!("{} ({})", d.name, d.path.display()))
            .collect();

        let device_names_for_closure = device_names.clone();
        let device_picker = row![
            text("Device:").size(14),
            pick_list(
                device_names.clone(),
                self.selected_device.map(|i| device_names[i].clone()),
                move |s| {
                    let idx = device_names_for_closure.iter().position(|n| *n == s).unwrap_or(0);
                    EventViewerMessage::DeviceSelected(idx)
                }
            ),
            button(text("Refresh"))
                .on_press(EventViewerMessage::RefreshDevices)
                .style(button::secondary),
        ]
        .spacing(10);

        let filter_options = vec![
            EventFilter::All,
            EventFilter::Keys,
            EventFilter::Axes,
            EventFilter::Buttons,
        ];

        let controls = row![
            if self.capturing {
                button(text("Stop"))
                    .on_press(EventViewerMessage::StopCapture)
                    .style(button::danger)
            } else {
                button(text("Start"))
                    .on_press_maybe(self.selected_device.map(|_| EventViewerMessage::StartCapture))
                    .style(button::success)
            },
            button(text(if self.paused { "Resume" } else { "Pause" }))
                .on_press(EventViewerMessage::TogglePause)
                .style(button::secondary),
            button(text("Clear"))
                .on_press(EventViewerMessage::ClearEvents)
                .style(button::secondary),
            pick_list(filter_options, Some(self.filter), EventViewerMessage::FilterChanged),
        ]
        .spacing(10);

        // Event table header
        let header = row![
            text("Time").size(12).width(Length::Fixed(120.0)),
            text("Type").size(12).width(Length::Fixed(80.0)),
            text("Code").size(12).width(Length::Fixed(60.0)),
            text("Name").size(12).width(Length::Fixed(150.0)),
            text("Value").size(12).width(Length::Fixed(100.0)),
        ]
        .spacing(10)
        .padding(5);

        // Event rows
        let mut event_list = Column::new().spacing(2);

        for event in &self.events {
            let event_row = row![
                text(&event.timestamp)
                    .size(11)
                    .width(Length::Fixed(120.0)),
                text(&event.event_type)
                    .size(11)
                    .width(Length::Fixed(80.0)),
                text(format!("{}", event.code))
                    .size(11)
                    .width(Length::Fixed(60.0)),
                text(&event.code_name)
                    .size(11)
                    .width(Length::Fixed(150.0)),
                text(&event.value_str)
                    .size(11)
                    .width(Length::Fixed(100.0)),
            ]
            .spacing(10)
            .padding(3);

            event_list = event_list.push(
                container(event_row).style(container::bordered_box),
            );
        }

        let status = if self.capturing {
            if self.paused {
                text("Paused").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(1.0, 0.8, 0.0)),
                })
            } else {
                text("Capturing...").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.0, 0.8, 0.0)),
                })
            }
        } else {
            text("Stopped").style(|_| text::Style {
                color: Some(iced::Color::from_rgb(0.5, 0.5, 0.5)),
            })
        };

        column![
            text("Event Viewer").size(20),
            device_picker,
            controls,
            row![status, text(format!("{} events", self.events.len())).size(12)].spacing(20),
            header,
            scrollable(event_list).height(Length::Fill),
        ]
        .spacing(10)
        .padding(20)
        .into()
    }
}

/// Get a human-readable name for an event code
fn get_code_name(event: &InputEvent) -> String {
    use crate::core::events::{abs_code_to_name, key_code_to_name, rel_code_to_name};

    match event.event_type {
        EventType::Key => key_code_to_name(event.code),
        EventType::Abs => abs_code_to_name(event.code),
        EventType::Rel => rel_code_to_name(event.code),
        _ => format!("CODE_{}", event.code),
    }
}

/// Get a human-readable value string
fn get_value_string(event: &InputEvent) -> String {
    match event.event_type {
        EventType::Key => match event.value {
            0 => "RELEASE".to_string(),
            1 => "PRESS".to_string(),
            2 => "REPEAT".to_string(),
            _ => format!("{}", event.value),
        },
        _ => format!("{}", event.value),
    }
}

impl Default for EventViewer {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
