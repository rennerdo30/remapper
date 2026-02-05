//! Profile editor view

use iced::widget::{button, column, container, row, scrollable, text, text_input, toggler, Column};
use iced::{Element, Length};

use crate::config::{DeviceMatch, Mapping, OutputConfig, Profile};
use crate::core::events::EventCode;

use super::app::Message;

/// Profile editor state
#[derive(Debug, Clone)]
pub struct ProfileEditor {
    /// Profile name
    pub name: String,
    /// Whether this is a new profile
    pub is_new: bool,
    /// Input device path or name
    pub input_device: String,
    /// Output device name
    pub output_name: String,
    /// Whether to grab the device
    pub grab: bool,
    /// Whether the profile is enabled
    pub enabled: bool,
    /// Mappings (simplified for now)
    pub mappings: Vec<MappingEntry>,
    /// Error message
    pub error: Option<String>,
}

/// Simplified mapping entry for the editor
#[derive(Debug, Clone)]
pub struct MappingEntry {
    pub from_code: String,
    pub to_code: String,
}

impl ProfileEditor {
    /// Create a new empty profile editor
    pub fn new() -> Self {
        Self {
            name: String::new(),
            is_new: true,
            input_device: String::new(),
            output_name: String::new(),
            grab: false,
            enabled: true,
            mappings: Vec::new(),
            error: None,
        }
    }

    /// Create an editor from an existing profile
    pub fn from_profile(profile: Profile) -> Self {
        let mappings = profile
            .mappings
            .iter()
            .filter_map(|m| match m {
                Mapping::Simple { from, to } => Some(MappingEntry {
                    from_code: from.code.clone(),
                    to_code: to.code.clone(),
                }),
                _ => None, // For now, only simple mappings in GUI
            })
            .collect();

        Self {
            name: profile.name,
            is_new: false,
            input_device: profile.input_device.display(),
            output_name: profile.output_device.name.unwrap_or_default(),
            grab: profile.grab,
            enabled: profile.enabled,
            mappings,
            error: None,
        }
    }

    /// Build a Profile from editor state
    pub fn build_profile(&self) -> Result<Profile, String> {
        if self.name.is_empty() {
            return Err("Profile name is required".to_string());
        }
        if self.input_device.is_empty() {
            return Err("Input device is required".to_string());
        }

        let input_device = if self.input_device.starts_with("/dev/") {
            DeviceMatch::by_path(&self.input_device)
        } else {
            DeviceMatch::by_name(&self.input_device)
        };

        let output_device = if self.output_name.is_empty() {
            OutputConfig::default()
        } else {
            OutputConfig::with_name(&self.output_name)
        };

        let mappings = self
            .mappings
            .iter()
            .filter(|m| !m.from_code.is_empty() && !m.to_code.is_empty())
            .map(|m| Mapping::simple(EventCode::key(&m.from_code), EventCode::key(&m.to_code)))
            .collect();

        Ok(Profile {
            name: self.name.clone(),
            enabled: self.enabled,
            input_device,
            output_device,
            grab: self.grab,
            mappings,
        })
    }

    /// Render the editor view
    pub fn view(&self) -> Element<'_, Message> {
        let title = if self.is_new {
            "Create New Profile"
        } else {
            "Edit Profile"
        };

        let name_input = column![
            text("Profile Name").size(14),
            text_input("Enter profile name...", &self.name)
                .padding(10)
                .size(14),
        ]
        .spacing(5);

        let device_input = column![
            text("Input Device").size(14),
            text("Enter device path (/dev/input/eventX) or name").size(11),
            text_input("Device path or name...", &self.input_device)
                .padding(10)
                .size(14),
        ]
        .spacing(5);

        let output_input = column![
            text("Virtual Device Name (optional)").size(14),
            text_input("Leave empty for default...", &self.output_name)
                .padding(10)
                .size(14),
        ]
        .spacing(5);

        let grab_toggle = row![
            text("Grab exclusive access").size(14),
            toggler(self.grab),
        ]
        .spacing(10);

        let enabled_toggle = row![
            text("Profile enabled").size(14),
            toggler(self.enabled),
        ]
        .spacing(10);

        // Mappings section
        let mut mappings_section = Column::new().spacing(5);
        mappings_section = mappings_section.push(text("Mappings").size(16));
        mappings_section =
            mappings_section.push(text("Add simple key remappings below").size(11));

        for mapping in self.mappings.iter() {
            let mapping_row = row![
                text_input("From (e.g., BTN_A)", &mapping.from_code)
                    .padding(8)
                    .size(12)
                    .width(Length::FillPortion(2)),
                text("→").size(14),
                text_input("To (e.g., BTN_B)", &mapping.to_code)
                    .padding(8)
                    .size(12)
                    .width(Length::FillPortion(2)),
                button(text("×")).style(button::danger),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            mappings_section = mappings_section.push(mapping_row);
        }

        mappings_section = mappings_section.push(
            button(text("+ Add Mapping"))
                .style(button::secondary)
                .padding(8),
        );

        // Error display
        let error_display = if let Some(err) = &self.error {
            container(text(err).style(|_| text::Style {
                color: Some(iced::Color::from_rgb(1.0, 0.3, 0.3)),
            }))
            .padding(10)
        } else {
            container(text(""))
        };

        // Action buttons
        let actions = row![
            button(text("Cancel"))
                .on_press(Message::CancelEdit)
                .style(button::secondary),
            button(text("Save"))
                .on_press_maybe(self.build_profile().ok().map(Message::SaveProfile))
                .style(button::primary),
        ]
        .spacing(10);

        let content = column![
            text(title).size(24),
            name_input,
            device_input,
            output_input,
            grab_toggle,
            enabled_toggle,
            container(mappings_section)
                .padding(10)
                .style(container::bordered_box),
            error_display,
            actions,
        ]
        .spacing(15)
        .padding(20)
        .max_width(600);

        scrollable(
            container(content)
                .center_x(Length::Fill)
                .padding(20),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

impl Default for ProfileEditor {
    fn default() -> Self {
        Self::new()
    }
}
