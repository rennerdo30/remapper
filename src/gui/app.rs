//! Main application state and logic

use iced::widget::{button, column, container, horizontal_space, row, scrollable, text, Column};
use iced::{Element, Length, Task, Theme};
use std::collections::HashMap;

use crate::config::{ConfigManager, Profile};
use crate::core::EngineState;
use crate::devices::DeviceManager;

use super::profile_editor::ProfileEditor;

/// Main application state
pub struct RemapperApp {
    /// Current view
    view: View,
    /// Configuration manager
    config: Option<ConfigManager>,
    /// Error message if config failed to load
    config_error: Option<String>,
    /// Running engines by profile name
    running: HashMap<String, EngineState>,
    /// Selected profile index
    selected_profile: Option<usize>,
    /// Profile editor state (when editing)
    profile_editor: Option<ProfileEditor>,
    /// Available devices
    devices: Vec<crate::devices::DeviceInfo>,
    /// Status message
    status: String,
}

/// Current view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Main profile list view
    Main,
    /// Profile editor
    Editor,
    /// Device list
    Devices,
    /// Event viewer/debugger
    Events,
}

/// Application messages
#[derive(Debug, Clone)]
pub enum Message {
    /// Profile selected
    ProfileSelected(usize),
    /// Start a profile
    StartProfile(String),
    /// Stop a profile
    StopProfile(String),
    /// Create new profile
    CreateProfile,
    /// Edit profile
    EditProfile(String),
    /// Delete profile
    DeleteProfile(String),
    /// Toggle profile enabled
    ToggleProfile(String),
    /// Save profile from editor
    SaveProfile(Profile),
    /// Cancel profile editing
    CancelEdit,
    /// Switch to view
    SwitchView(View),
    /// Refresh device list
    RefreshDevices,
    /// Device list loaded
    DevicesLoaded(Vec<crate::devices::DeviceInfo>),
    /// Reload configuration
    ReloadConfig,
    /// Config loaded
    ConfigLoaded(Result<ConfigManager, String>),
    /// Engine state changed
    EngineStateChanged(String, EngineState),
    /// Error occurred
    Error(String),
    /// Clear status
    ClearStatus,
}

impl RemapperApp {
    pub fn new() -> (Self, Task<Message>) {
        let app = Self {
            view: View::Main,
            config: None,
            config_error: None,
            running: HashMap::new(),
            selected_profile: None,
            profile_editor: None,
            devices: Vec::new(),
            status: "Loading...".to_string(),
        };

        // Load config on startup
        let task = Task::perform(async { load_config().await }, |result| {
            Message::ConfigLoaded(result)
        });

        (app, task)
    }

    pub fn title(&self) -> String {
        "Remapper".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ProfileSelected(idx) => {
                self.selected_profile = Some(idx);
                Task::none()
            }

            Message::StartProfile(name) => {
                self.status = format!("Starting {}...", name);
                self.running.insert(name.clone(), EngineState::Starting);
                // TODO: Actually start the engine
                Task::perform(async move { (name, EngineState::Running) }, |(n, s)| {
                    Message::EngineStateChanged(n, s)
                })
            }

            Message::StopProfile(name) => {
                self.status = format!("Stopping {}...", name);
                self.running.insert(name.clone(), EngineState::Stopping);
                Task::perform(async move { (name, EngineState::Stopped) }, |(n, s)| {
                    Message::EngineStateChanged(n, s)
                })
            }

            Message::CreateProfile => {
                self.profile_editor = Some(ProfileEditor::new());
                self.view = View::Editor;
                Task::none()
            }

            Message::EditProfile(name) => {
                if let Some(config) = &self.config {
                    if let Some(profile) = config.get_profile(&name) {
                        self.profile_editor = Some(ProfileEditor::from_profile(profile.clone()));
                        self.view = View::Editor;
                    }
                }
                Task::none()
            }

            Message::DeleteProfile(name) => {
                if let Some(config) = &mut self.config {
                    match config.delete_profile(&name) {
                        Ok(()) => {
                            self.status = format!("Deleted profile: {}", name);
                            self.selected_profile = None;
                        }
                        Err(e) => {
                            self.status = format!("Error: {}", e);
                        }
                    }
                }
                Task::none()
            }

            Message::ToggleProfile(name) => {
                if let Some(config) = &mut self.config {
                    let new_enabled = if let Some(profile) = config.config_mut().find_profile_mut(&name) {
                        profile.enabled = !profile.enabled;
                        Some(profile.enabled)
                    } else {
                        None
                    };
                    if let Some(enabled) = new_enabled {
                        let _ = config.save();
                        self.status = if enabled {
                            format!("Enabled: {}", name)
                        } else {
                            format!("Disabled: {}", name)
                        };
                    }
                }
                Task::none()
            }

            Message::SaveProfile(profile) => {
                if let Some(config) = &mut self.config {
                    let is_new = config.get_profile(&profile.name).is_none();
                    if is_new {
                        match config.add_profile(profile.clone()) {
                            Ok(()) => {
                                self.status = format!("Created profile: {}", profile.name);
                            }
                            Err(e) => {
                                self.status = format!("Error: {}", e);
                            }
                        }
                    } else {
                        match config.update_profile(&profile.name, profile.clone()) {
                            Ok(()) => {
                                self.status = format!("Updated profile: {}", profile.name);
                            }
                            Err(e) => {
                                self.status = format!("Error: {}", e);
                            }
                        }
                    }
                }
                self.profile_editor = None;
                self.view = View::Main;
                Task::none()
            }

            Message::CancelEdit => {
                self.profile_editor = None;
                self.view = View::Main;
                Task::none()
            }

            Message::SwitchView(view) => {
                self.view = view;
                if view == View::Devices {
                    return Task::perform(async { refresh_devices().await }, |devices| {
                        Message::DevicesLoaded(devices)
                    });
                }
                Task::none()
            }

            Message::RefreshDevices => {
                Task::perform(async { refresh_devices().await }, |devices| {
                    Message::DevicesLoaded(devices)
                })
            }

            Message::DevicesLoaded(devices) => {
                self.devices = devices;
                self.status = format!("Found {} devices", self.devices.len());
                Task::none()
            }

            Message::ReloadConfig => {
                Task::perform(async { load_config().await }, |result| {
                    Message::ConfigLoaded(result)
                })
            }

            Message::ConfigLoaded(result) => {
                match result {
                    Ok(config) => {
                        let count = config.profiles().len();
                        self.config = Some(config);
                        self.config_error = None;
                        self.status = format!("Loaded {} profiles", count);
                    }
                    Err(e) => {
                        self.config_error = Some(e.clone());
                        self.status = format!("Error: {}", e);
                    }
                }
                Task::none()
            }

            Message::EngineStateChanged(name, state) => {
                if state == EngineState::Stopped {
                    self.running.remove(&name);
                } else {
                    self.running.insert(name.clone(), state);
                }
                self.status = format!("{}: {}", name, state);
                Task::none()
            }

            Message::Error(msg) => {
                self.status = format!("Error: {}", msg);
                Task::none()
            }

            Message::ClearStatus => {
                self.status.clear();
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<Message> = match self.view {
            View::Main => self.view_main(),
            View::Editor => self.view_editor(),
            View::Devices => self.view_devices(),
            View::Events => self.view_events(),
        };

        // Main layout with toolbar and content
        let toolbar = self.view_toolbar();

        let status_bar = container(text(&self.status).size(12))
            .padding(5)
            .width(Length::Fill)
            .style(container::bordered_box);

        let layout = column![toolbar, content, status_bar]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}

impl RemapperApp {
    /// Render the toolbar
    fn view_toolbar(&self) -> Element<'_, Message> {
        let profiles_btn = button(text("Profiles"))
            .on_press(Message::SwitchView(View::Main))
            .style(if self.view == View::Main {
                button::primary
            } else {
                button::secondary
            });

        let devices_btn = button(text("Devices"))
            .on_press(Message::SwitchView(View::Devices))
            .style(if self.view == View::Devices {
                button::primary
            } else {
                button::secondary
            });

        let events_btn = button(text("Events"))
            .on_press(Message::SwitchView(View::Events))
            .style(if self.view == View::Events {
                button::primary
            } else {
                button::secondary
            });

        let add_btn = button(text("+ New Profile"))
            .on_press(Message::CreateProfile)
            .style(button::success);

        let refresh_btn = button(text("Refresh"))
            .on_press(Message::ReloadConfig)
            .style(button::secondary);

        let toolbar = row![
            profiles_btn,
            devices_btn,
            events_btn,
            horizontal_space(),
            add_btn,
            refresh_btn,
        ]
        .spacing(10)
        .padding(10);

        container(toolbar)
            .width(Length::Fill)
            .style(container::bordered_box)
            .into()
    }

    /// Render the main profile list view
    fn view_main(&self) -> Element<'_, Message> {
        if let Some(error) = &self.config_error {
            return container(
                column![
                    text("Configuration Error").size(24),
                    text(error).size(14),
                    button(text("Retry")).on_press(Message::ReloadConfig),
                ]
                .spacing(20)
                .padding(40),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }

        let profiles = self
            .config
            .as_ref()
            .map(|c| c.profiles())
            .unwrap_or(&[]);

        if profiles.is_empty() {
            return container(
                column![
                    text("No Profiles Configured").size(24),
                    text("Create a profile to get started.").size(14),
                    button(text("Create Profile")).on_press(Message::CreateProfile),
                ]
                .spacing(20)
                .padding(40),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }

        let mut profile_list = Column::new().spacing(5).padding(10);

        for (idx, profile) in profiles.iter().enumerate() {
            let is_selected = self.selected_profile == Some(idx);
            let is_running = self
                .running
                .get(&profile.name)
                .map(|s| *s == EngineState::Running)
                .unwrap_or(false);

            let status_indicator = if is_running {
                text("●").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.0, 0.8, 0.0)),
                })
            } else if profile.enabled {
                text("○").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.5, 0.5, 0.5)),
                })
            } else {
                text("○").style(|_| text::Style {
                    color: Some(iced::Color::from_rgb(0.3, 0.3, 0.3)),
                })
            };

            let name = text(&profile.name).size(16);
            let device = text(profile.input_device.display()).size(12);
            let mappings = text(format!("{} mappings", profile.mappings.len())).size(12);

            let info = column![name, device, mappings].spacing(2);

            let start_stop = if is_running {
                button(text("Stop"))
                    .on_press(Message::StopProfile(profile.name.clone()))
                    .style(button::danger)
            } else {
                button(text("Start"))
                    .on_press(Message::StartProfile(profile.name.clone()))
                    .style(button::success)
            };

            let edit_btn = button(text("Edit"))
                .on_press(Message::EditProfile(profile.name.clone()))
                .style(button::secondary);

            let toggle_btn = button(text(if profile.enabled { "Disable" } else { "Enable" }))
                .on_press(Message::ToggleProfile(profile.name.clone()))
                .style(button::secondary);

            let profile_row = row![
                status_indicator,
                info,
                horizontal_space(),
                start_stop,
                edit_btn,
                toggle_btn,
            ]
            .spacing(10)
            .padding(10)
            .align_y(iced::Alignment::Center);

            let profile_container = container(profile_row)
                .width(Length::Fill)
                .style(if is_selected {
                    container::bordered_box
                } else {
                    container::transparent
                });

            let clickable = button(profile_container)
                .on_press(Message::ProfileSelected(idx))
                .style(button::text)
                .width(Length::Fill);

            profile_list = profile_list.push(clickable);
        }

        scrollable(profile_list)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Render the profile editor
    fn view_editor(&self) -> Element<'_, Message> {
        if let Some(editor) = &self.profile_editor {
            editor.view()
        } else {
            container(text("No profile being edited"))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        }
    }

    /// Render the device list
    fn view_devices(&self) -> Element<'_, Message> {
        let mut device_list = Column::new().spacing(10).padding(20);

        device_list = device_list.push(
            row![
                text("Available Input Devices").size(20),
                horizontal_space(),
                button(text("Refresh"))
                    .on_press(Message::RefreshDevices)
                    .style(button::secondary),
            ]
            .spacing(10),
        );

        if self.devices.is_empty() {
            device_list = device_list.push(
                container(text("No devices found. Click Refresh to scan.").size(14))
                    .padding(20),
            );
        } else {
            for device in &self.devices {
                let type_label = if device.is_gamepad {
                    text("[Gamepad]").style(|_| text::Style {
                        color: Some(iced::Color::from_rgb(0.0, 0.8, 0.0)),
                    })
                } else if device.is_keyboard {
                    text("[Keyboard]").style(|_| text::Style {
                        color: Some(iced::Color::from_rgb(0.0, 0.6, 1.0)),
                    })
                } else if device.is_mouse {
                    text("[Mouse]").style(|_| text::Style {
                        color: Some(iced::Color::from_rgb(1.0, 0.8, 0.0)),
                    })
                } else {
                    text("[Other]").style(|_| text::Style {
                        color: Some(iced::Color::from_rgb(0.5, 0.5, 0.5)),
                    })
                };

                let device_row = row![
                    type_label,
                    column![
                        text(&device.name).size(14),
                        text(format!("{}", device.path.display())).size(11),
                        text(format!(
                            "ID: {:04x}:{:04x}",
                            device.vendor, device.product
                        ))
                        .size(11),
                    ]
                    .spacing(2),
                ]
                .spacing(15)
                .padding(10);

                device_list = device_list.push(
                    container(device_row)
                        .width(Length::Fill)
                        .style(container::bordered_box),
                );
            }
        }

        scrollable(device_list)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Render the event viewer
    fn view_events(&self) -> Element<'_, Message> {
        container(
            column![
                text("Event Viewer").size(20),
                text("Select a device to view its events.").size(14),
                text("(Not yet implemented in GUI - use 'remapper debug <device>' in terminal)")
                    .size(12),
            ]
            .spacing(10)
            .padding(40),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    }
}

impl Default for RemapperApp {
    fn default() -> Self {
        Self::new().0
    }
}

/// Load configuration asynchronously
async fn load_config() -> Result<ConfigManager, String> {
    ConfigManager::load().map_err(|e| e.to_string())
}

/// Refresh device list asynchronously
async fn refresh_devices() -> Vec<crate::devices::DeviceInfo> {
    DeviceManager::list_devices().unwrap_or_default()
}
