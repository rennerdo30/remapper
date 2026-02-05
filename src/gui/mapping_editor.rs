//! Visual mapping editor component

use iced::widget::{button, column, container, pick_list, row, text, text_input, Column};
use iced::{Element, Length};

use crate::config::{MacroStep, Mapping};
use crate::core::events::EventCode;

/// Mapping editor component
pub struct MappingEditor {
    /// Current mapping being edited
    mapping: MappingType,
    /// From event code
    from_code: String,
    /// To event code
    to_code: String,
    /// Macro steps
    macro_steps: Vec<MacroStepEntry>,
    /// Tap code (for conditional)
    tap_code: String,
    /// Hold code (for conditional)
    hold_code: String,
    /// Hold threshold
    threshold_ms: String,
    /// Combo keys
    combo_keys: Vec<String>,
    /// Error message
    error: Option<String>,
}

/// Entry for a macro step
#[derive(Debug, Clone)]
pub struct MacroStepEntry {
    pub step_type: MacroStepType,
    pub code: String,
    pub delay_ms: String,
}

/// Type of macro step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroStepType {
    Press,
    Release,
    Delay,
}

impl std::fmt::Display for MacroStepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacroStepType::Press => write!(f, "Press"),
            MacroStepType::Release => write!(f, "Release"),
            MacroStepType::Delay => write!(f, "Delay"),
        }
    }
}

/// Type of mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingType {
    Simple,
    Macro,
    Conditional,
    Combo,
}

impl std::fmt::Display for MappingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MappingType::Simple => write!(f, "Simple (1:1)"),
            MappingType::Macro => write!(f, "Macro (sequence)"),
            MappingType::Conditional => write!(f, "Tap/Hold"),
            MappingType::Combo => write!(f, "Combo (chord)"),
        }
    }
}

/// Mapping editor messages
#[derive(Debug, Clone)]
pub enum MappingEditorMessage {
    /// Mapping type changed
    TypeChanged(MappingType),
    /// From code changed
    FromCodeChanged(String),
    /// To code changed
    ToCodeChanged(String),
    /// Tap code changed
    TapCodeChanged(String),
    /// Hold code changed
    HoldCodeChanged(String),
    /// Threshold changed
    ThresholdChanged(String),
    /// Add macro step
    AddMacroStep,
    /// Remove macro step
    RemoveMacroStep(usize),
    /// Macro step type changed
    MacroStepTypeChanged(usize, MacroStepType),
    /// Macro step code changed
    MacroStepCodeChanged(usize, String),
    /// Macro step delay changed
    MacroStepDelayChanged(usize, String),
    /// Add combo key
    AddComboKey,
    /// Remove combo key
    RemoveComboKey(usize),
    /// Combo key changed
    ComboKeyChanged(usize, String),
    /// Save mapping
    Save,
    /// Cancel
    Cancel,
}

impl MappingEditor {
    /// Create a new empty mapping editor
    pub fn new() -> Self {
        Self {
            mapping: MappingType::Simple,
            from_code: String::new(),
            to_code: String::new(),
            macro_steps: Vec::new(),
            tap_code: String::new(),
            hold_code: String::new(),
            threshold_ms: "300".to_string(),
            combo_keys: vec![String::new(), String::new()],
            error: None,
        }
    }

    /// Create an editor from an existing mapping
    pub fn from_mapping(mapping: &Mapping) -> Self {
        match mapping {
            Mapping::Simple { from, to } => Self {
                mapping: MappingType::Simple,
                from_code: from.code.clone(),
                to_code: to.code.clone(),
                ..Self::new()
            },
            Mapping::Macro { trigger, sequence } => {
                let macro_steps = sequence
                    .iter()
                    .map(|step| match step {
                        MacroStep::Key { code, value } => MacroStepEntry {
                            step_type: if *value == 1 {
                                MacroStepType::Press
                            } else {
                                MacroStepType::Release
                            },
                            code: code.clone(),
                            delay_ms: String::new(),
                        },
                        MacroStep::Delay { delay_ms } => MacroStepEntry {
                            step_type: MacroStepType::Delay,
                            code: String::new(),
                            delay_ms: delay_ms.to_string(),
                        },
                    })
                    .collect();

                Self {
                    mapping: MappingType::Macro,
                    from_code: trigger.code.clone(),
                    macro_steps,
                    ..Self::new()
                }
            }
            Mapping::Conditional {
                trigger,
                tap,
                hold,
                threshold_ms,
            } => Self {
                mapping: MappingType::Conditional,
                from_code: trigger.code.clone(),
                tap_code: tap.as_ref().map(|c| c.code.clone()).unwrap_or_default(),
                hold_code: hold.as_ref().map(|c| c.code.clone()).unwrap_or_default(),
                threshold_ms: threshold_ms.to_string(),
                ..Self::new()
            },
            Mapping::Combo { keys, output, .. } => {
                let combo_keys = keys.iter().map(|k| k.code.clone()).collect();
                Self {
                    mapping: MappingType::Combo,
                    to_code: output.code.clone(),
                    combo_keys,
                    ..Self::new()
                }
            }
        }
    }

    /// Build a Mapping from editor state
    pub fn build_mapping(&self) -> Result<Mapping, String> {
        match self.mapping {
            MappingType::Simple => {
                if self.from_code.is_empty() {
                    return Err("From code is required".to_string());
                }
                if self.to_code.is_empty() {
                    return Err("To code is required".to_string());
                }
                Ok(Mapping::simple(
                    EventCode::key(&self.from_code),
                    EventCode::key(&self.to_code),
                ))
            }
            MappingType::Macro => {
                if self.from_code.is_empty() {
                    return Err("Trigger code is required".to_string());
                }
                if self.macro_steps.is_empty() {
                    return Err("At least one macro step is required".to_string());
                }

                let steps: Result<Vec<MacroStep>, String> = self
                    .macro_steps
                    .iter()
                    .map(|step| match step.step_type {
                        MacroStepType::Press => {
                            if step.code.is_empty() {
                                Err("Key code is required".to_string())
                            } else {
                                Ok(MacroStep::press(&step.code))
                            }
                        }
                        MacroStepType::Release => {
                            if step.code.is_empty() {
                                Err("Key code is required".to_string())
                            } else {
                                Ok(MacroStep::release(&step.code))
                            }
                        }
                        MacroStepType::Delay => {
                            let ms = step.delay_ms.parse::<u32>().map_err(|_| "Invalid delay")?;
                            Ok(MacroStep::delay(ms))
                        }
                    })
                    .collect();

                Ok(Mapping::macro_seq(EventCode::key(&self.from_code), steps?))
            }
            MappingType::Conditional => {
                if self.from_code.is_empty() {
                    return Err("Trigger code is required".to_string());
                }
                if self.tap_code.is_empty() && self.hold_code.is_empty() {
                    return Err("At least tap or hold must be specified".to_string());
                }

                let threshold = self
                    .threshold_ms
                    .parse::<u32>()
                    .map_err(|_| "Invalid threshold")?;

                let tap = if self.tap_code.is_empty() {
                    None
                } else {
                    Some(EventCode::key(&self.tap_code))
                };

                let hold = if self.hold_code.is_empty() {
                    None
                } else {
                    Some(EventCode::key(&self.hold_code))
                };

                Ok(Mapping::conditional(
                    EventCode::key(&self.from_code),
                    tap,
                    hold,
                    threshold,
                ))
            }
            MappingType::Combo => {
                if self.to_code.is_empty() {
                    return Err("Output code is required".to_string());
                }
                let keys: Vec<EventCode> = self
                    .combo_keys
                    .iter()
                    .filter(|k| !k.is_empty())
                    .map(EventCode::key)
                    .collect();

                if keys.len() < 2 {
                    return Err("At least 2 combo keys are required".to_string());
                }

                Ok(Mapping::combo(keys, EventCode::key(&self.to_code)))
            }
        }
    }

    /// Handle message
    pub fn update(&mut self, message: MappingEditorMessage) {
        match message {
            MappingEditorMessage::TypeChanged(t) => self.mapping = t,
            MappingEditorMessage::FromCodeChanged(s) => self.from_code = s,
            MappingEditorMessage::ToCodeChanged(s) => self.to_code = s,
            MappingEditorMessage::TapCodeChanged(s) => self.tap_code = s,
            MappingEditorMessage::HoldCodeChanged(s) => self.hold_code = s,
            MappingEditorMessage::ThresholdChanged(s) => self.threshold_ms = s,
            MappingEditorMessage::AddMacroStep => {
                self.macro_steps.push(MacroStepEntry {
                    step_type: MacroStepType::Press,
                    code: String::new(),
                    delay_ms: "50".to_string(),
                });
            }
            MappingEditorMessage::RemoveMacroStep(idx) => {
                if idx < self.macro_steps.len() {
                    self.macro_steps.remove(idx);
                }
            }
            MappingEditorMessage::MacroStepTypeChanged(idx, t) => {
                if let Some(step) = self.macro_steps.get_mut(idx) {
                    step.step_type = t;
                }
            }
            MappingEditorMessage::MacroStepCodeChanged(idx, s) => {
                if let Some(step) = self.macro_steps.get_mut(idx) {
                    step.code = s;
                }
            }
            MappingEditorMessage::MacroStepDelayChanged(idx, s) => {
                if let Some(step) = self.macro_steps.get_mut(idx) {
                    step.delay_ms = s;
                }
            }
            MappingEditorMessage::AddComboKey => {
                self.combo_keys.push(String::new());
            }
            MappingEditorMessage::RemoveComboKey(idx) => {
                if self.combo_keys.len() > 2 && idx < self.combo_keys.len() {
                    self.combo_keys.remove(idx);
                }
            }
            MappingEditorMessage::ComboKeyChanged(idx, s) => {
                if let Some(key) = self.combo_keys.get_mut(idx) {
                    *key = s;
                }
            }
            MappingEditorMessage::Save | MappingEditorMessage::Cancel => {
                // Parent handles these
            }
        }
    }

    /// Render the mapping editor
    pub fn view(&self) -> Element<'_, MappingEditorMessage> {
        let type_options = vec![
            MappingType::Simple,
            MappingType::Macro,
            MappingType::Conditional,
            MappingType::Combo,
        ];

        let type_picker = row![
            text("Mapping Type:").size(14),
            pick_list(type_options, Some(self.mapping), MappingEditorMessage::TypeChanged),
        ]
        .spacing(10);

        let fields: Element<MappingEditorMessage> = match self.mapping {
            MappingType::Simple => self.view_simple_fields(),
            MappingType::Macro => self.view_macro_fields(),
            MappingType::Conditional => self.view_conditional_fields(),
            MappingType::Combo => self.view_combo_fields(),
        };

        let error = if let Some(e) = &self.error {
            container(text(e).style(|_| text::Style {
                color: Some(iced::Color::from_rgb(1.0, 0.3, 0.3)),
            }))
        } else {
            container(text(""))
        };

        let actions = row![
            button(text("Cancel"))
                .on_press(MappingEditorMessage::Cancel)
                .style(button::secondary),
            button(text("Save"))
                .on_press(MappingEditorMessage::Save)
                .style(button::primary),
        ]
        .spacing(10);

        column![type_picker, fields, error, actions]
            .spacing(15)
            .padding(10)
            .into()
    }

    fn view_simple_fields(&self) -> Element<'_, MappingEditorMessage> {
        column![
            row![
                text("From:").size(14).width(Length::Fixed(60.0)),
                text_input("e.g., BTN_A", &self.from_code)
                    .on_input(MappingEditorMessage::FromCodeChanged)
                    .padding(8),
            ]
            .spacing(10),
            row![
                text("To:").size(14).width(Length::Fixed(60.0)),
                text_input("e.g., BTN_B", &self.to_code)
                    .on_input(MappingEditorMessage::ToCodeChanged)
                    .padding(8),
            ]
            .spacing(10),
        ]
        .spacing(10)
        .into()
    }

    fn view_macro_fields(&self) -> Element<'_, MappingEditorMessage> {
        let mut steps = Column::new().spacing(5);

        for (idx, step) in self.macro_steps.iter().enumerate() {
            let step_types = vec![
                MacroStepType::Press,
                MacroStepType::Release,
                MacroStepType::Delay,
            ];

            let step_row = row![
                pick_list(
                    step_types,
                    Some(step.step_type),
                    move |t| MappingEditorMessage::MacroStepTypeChanged(idx, t)
                ),
                if step.step_type == MacroStepType::Delay {
                    text_input("ms", &step.delay_ms)
                        .on_input(move |s| MappingEditorMessage::MacroStepDelayChanged(idx, s))
                        .padding(8)
                        .width(Length::Fixed(80.0))
                } else {
                    text_input("Key code", &step.code)
                        .on_input(move |s| MappingEditorMessage::MacroStepCodeChanged(idx, s))
                        .padding(8)
                },
                button(text("×"))
                    .on_press(MappingEditorMessage::RemoveMacroStep(idx))
                    .style(button::danger),
            ]
            .spacing(10);

            steps = steps.push(step_row);
        }

        column![
            row![
                text("Trigger:").size(14).width(Length::Fixed(60.0)),
                text_input("e.g., BTN_SELECT", &self.from_code)
                    .on_input(MappingEditorMessage::FromCodeChanged)
                    .padding(8),
            ]
            .spacing(10),
            text("Steps:").size(14),
            steps,
            button(text("+ Add Step"))
                .on_press(MappingEditorMessage::AddMacroStep)
                .style(button::secondary),
        ]
        .spacing(10)
        .into()
    }

    fn view_conditional_fields(&self) -> Element<'_, MappingEditorMessage> {
        column![
            row![
                text("Trigger:").size(14).width(Length::Fixed(80.0)),
                text_input("e.g., BTN_START", &self.from_code)
                    .on_input(MappingEditorMessage::FromCodeChanged)
                    .padding(8),
            ]
            .spacing(10),
            row![
                text("Tap:").size(14).width(Length::Fixed(80.0)),
                text_input("e.g., KEY_ESC", &self.tap_code)
                    .on_input(MappingEditorMessage::TapCodeChanged)
                    .padding(8),
            ]
            .spacing(10),
            row![
                text("Hold:").size(14).width(Length::Fixed(80.0)),
                text_input("e.g., KEY_LEFTMETA", &self.hold_code)
                    .on_input(MappingEditorMessage::HoldCodeChanged)
                    .padding(8),
            ]
            .spacing(10),
            row![
                text("Threshold:").size(14).width(Length::Fixed(80.0)),
                text_input("ms", &self.threshold_ms)
                    .on_input(MappingEditorMessage::ThresholdChanged)
                    .padding(8)
                    .width(Length::Fixed(80.0)),
                text("ms").size(14),
            ]
            .spacing(10),
        ]
        .spacing(10)
        .into()
    }

    fn view_combo_fields(&self) -> Element<'_, MappingEditorMessage> {
        let mut keys = Column::new().spacing(5);

        for (idx, key) in self.combo_keys.iter().enumerate() {
            let key_row = row![
                text_input("Key code", key)
                    .on_input(move |s| MappingEditorMessage::ComboKeyChanged(idx, s))
                    .padding(8),
                button(text("×"))
                    .on_press(MappingEditorMessage::RemoveComboKey(idx))
                    .style(button::danger),
            ]
            .spacing(10);

            keys = keys.push(key_row);
        }

        column![
            text("Combo Keys:").size(14),
            keys,
            button(text("+ Add Key"))
                .on_press(MappingEditorMessage::AddComboKey)
                .style(button::secondary),
            row![
                text("Output:").size(14).width(Length::Fixed(60.0)),
                text_input("e.g., KEY_ESC", &self.to_code)
                    .on_input(MappingEditorMessage::ToCodeChanged)
                    .padding(8),
            ]
            .spacing(10),
        ]
        .spacing(10)
        .into()
    }
}

impl Default for MappingEditor {
    fn default() -> Self {
        Self::new()
    }
}
