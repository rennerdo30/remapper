//! Device picker component

use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Element, Length};

use crate::devices::DeviceInfo;

/// Device picker component
pub struct DevicePicker {
    /// Available devices
    devices: Vec<DeviceInfo>,
    /// Selected device index
    selected: Option<usize>,
    /// Filter by device type
    filter: DeviceFilter,
}

/// Device type filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceFilter {
    All,
    Gamepads,
    Keyboards,
    Mice,
}

/// Device picker messages
#[derive(Debug, Clone)]
pub enum DevicePickerMessage {
    /// Device selected
    Selected(usize),
    /// Filter changed
    FilterChanged(DeviceFilter),
    /// Refresh devices
    Refresh,
    /// Confirm selection
    Confirm,
    /// Cancel selection
    Cancel,
}

impl DevicePicker {
    /// Create a new device picker
    pub fn new(devices: Vec<DeviceInfo>) -> Self {
        Self {
            devices,
            selected: None,
            filter: DeviceFilter::All,
        }
    }

    /// Get filtered devices
    fn filtered_devices(&self) -> Vec<&DeviceInfo> {
        self.devices
            .iter()
            .filter(|d| match self.filter {
                DeviceFilter::All => true,
                DeviceFilter::Gamepads => d.is_gamepad,
                DeviceFilter::Keyboards => d.is_keyboard,
                DeviceFilter::Mice => d.is_mouse,
            })
            .collect()
    }

    /// Get the selected device
    pub fn selected_device(&self) -> Option<&DeviceInfo> {
        self.selected.and_then(|idx| {
            self.filtered_devices().get(idx).copied()
        })
    }

    /// Handle message
    pub fn update(&mut self, message: DevicePickerMessage) {
        match message {
            DevicePickerMessage::Selected(idx) => {
                self.selected = Some(idx);
            }
            DevicePickerMessage::FilterChanged(filter) => {
                self.filter = filter;
                self.selected = None;
            }
            DevicePickerMessage::Refresh => {
                // Parent should refresh devices
            }
            DevicePickerMessage::Confirm | DevicePickerMessage::Cancel => {
                // Parent handles these
            }
        }
    }

    /// Render the device picker
    pub fn view(&self) -> Element<'_, DevicePickerMessage> {
        let filter_buttons = row![
            filter_button("All", DeviceFilter::All, self.filter),
            filter_button("Gamepads", DeviceFilter::Gamepads, self.filter),
            filter_button("Keyboards", DeviceFilter::Keyboards, self.filter),
            filter_button("Mice", DeviceFilter::Mice, self.filter),
        ]
        .spacing(5);

        let devices = self.filtered_devices();
        let mut device_list = Column::new().spacing(5);

        for (idx, device) in devices.iter().enumerate() {
            let is_selected = self.selected == Some(idx);

            let type_badge = if device.is_gamepad {
                "[Gamepad]"
            } else if device.is_keyboard {
                "[Keyboard]"
            } else if device.is_mouse {
                "[Mouse]"
            } else {
                "[Other]"
            };

            let device_row = row![
                text(type_badge).size(12),
                column![
                    text(&device.name).size(14),
                    text(format!("{}", device.path.display())).size(11),
                ]
                .spacing(2),
            ]
            .spacing(10)
            .padding(8);

            let device_container = container(device_row)
                .width(Length::Fill)
                .style(if is_selected {
                    container::bordered_box
                } else {
                    container::transparent
                });

            let clickable = button(device_container)
                .on_press(DevicePickerMessage::Selected(idx))
                .style(button::text)
                .width(Length::Fill);

            device_list = device_list.push(clickable);
        }

        let actions = row![
            button(text("Cancel"))
                .on_press(DevicePickerMessage::Cancel)
                .style(button::secondary),
            button(text("Refresh"))
                .on_press(DevicePickerMessage::Refresh)
                .style(button::secondary),
            button(text("Select"))
                .on_press_maybe(self.selected.map(|_| DevicePickerMessage::Confirm))
                .style(button::primary),
        ]
        .spacing(10);

        let content = column![
            text("Select Input Device").size(20),
            filter_buttons,
            scrollable(device_list).height(Length::Fixed(300.0)),
            actions,
        ]
        .spacing(15)
        .padding(20);

        container(content)
            .width(Length::Fill)
            .style(container::bordered_box)
            .into()
    }
}

fn filter_button<'a>(
    label: &'a str,
    filter: DeviceFilter,
    current: DeviceFilter,
) -> Element<'a, DevicePickerMessage> {
    button(text(label).size(12))
        .on_press(DevicePickerMessage::FilterChanged(filter))
        .style(if filter == current {
            button::primary
        } else {
            button::secondary
        })
        .padding(5)
        .into()
}
