//! Virtual output device using uinput

use evdev::uinput::VirtualDevice;
use evdev::{AbsInfo, AbsoluteAxisCode, AttributeSet, KeyCode, RelativeAxisCode, UinputAbsSetup};
use std::sync::Mutex;
use tracing::{debug, trace};

use crate::core::error::{RemapperError, Result};
use crate::core::events::InputEvent;
use crate::devices::InputDevice;

/// Virtual output device
pub struct OutputDevice {
    /// The uinput virtual device
    device: Mutex<VirtualDevice>,
    /// Device name
    name: String,
}

impl OutputDevice {
    /// Create a new virtual output device with capabilities from an input device
    pub fn create(name: &str, input: &InputDevice) -> Result<Self> {
        let mut builder = VirtualDevice::builder()
            .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?
            .name(name);

        // Copy supported keys from input device
        let keys = input.supported_keys();
        if !keys.is_empty() {
            let mut key_set = AttributeSet::<KeyCode>::new();
            for key in keys {
                key_set.insert(key);
            }
            builder = builder
                .with_keys(&key_set)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
        }

        // Copy supported relative axes
        let rel_axes = input.supported_relative_axes();
        if !rel_axes.is_empty() {
            let mut rel_set = AttributeSet::<RelativeAxisCode>::new();
            for axis in rel_axes {
                rel_set.insert(axis);
            }
            builder = builder
                .with_relative_axes(&rel_set)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
        }

        // Copy supported absolute axes with their info
        let abs_axes = input.supported_absolute_axes();
        for axis in abs_axes {
            if let Some(info) = input.abs_info(axis) {
                let abs_setup = UinputAbsSetup::new(
                    axis,
                    AbsInfo::new(
                        info.value(),
                        info.minimum(),
                        info.maximum(),
                        info.fuzz(),
                        info.flat(),
                        info.resolution(),
                    ),
                );
                builder = builder
                    .with_absolute_axis(&abs_setup)
                    .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
            }
        }

        let device = builder
            .build()
            .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;

        debug!("Created virtual device: {}", name);

        Ok(Self {
            device: Mutex::new(device),
            name: name.to_string(),
        })
    }

    /// Create a new virtual output device with specified capabilities
    pub fn create_with_caps(
        name: &str,
        keys: &[KeyCode],
        rel_axes: &[RelativeAxisCode],
        abs_axes: &[(AbsoluteAxisCode, AbsInfo)],
    ) -> Result<Self> {
        let mut builder = VirtualDevice::builder()
            .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?
            .name(name);

        if !keys.is_empty() {
            let mut key_set = AttributeSet::<KeyCode>::new();
            for key in keys {
                key_set.insert(*key);
            }
            builder = builder
                .with_keys(&key_set)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
        }

        if !rel_axes.is_empty() {
            let mut rel_set = AttributeSet::<RelativeAxisCode>::new();
            for axis in rel_axes {
                rel_set.insert(*axis);
            }
            builder = builder
                .with_relative_axes(&rel_set)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
        }

        for (axis, info) in abs_axes {
            let abs_setup = UinputAbsSetup::new(*axis, *info);
            builder = builder
                .with_absolute_axis(&abs_setup)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
        }

        let device = builder
            .build()
            .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;

        debug!("Created virtual device with custom caps: {}", name);

        Ok(Self {
            device: Mutex::new(device),
            name: name.to_string(),
        })
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Write an event to the virtual device
    pub fn write_event(&self, event: &InputEvent) -> Result<()> {
        let evdev_event = event.to_evdev();
        trace!("Writing event: {}", event);
        let mut device = self.device.lock().map_err(|_| {
            RemapperError::EventWriteError("Output device lock poisoned".to_string())
        })?;
        device
            .emit(&[evdev_event])
            .map_err(|e| RemapperError::EventWriteError(e.to_string()))?;
        Ok(())
    }

    /// Write multiple events to the virtual device
    pub fn write_events(&self, events: &[InputEvent]) -> Result<()> {
        let evdev_events: Vec<_> = events.iter().map(|e| e.to_evdev()).collect();
        let mut device = self.device.lock().map_err(|_| {
            RemapperError::EventWriteError("Output device lock poisoned".to_string())
        })?;
        device
            .emit(&evdev_events)
            .map_err(|e| RemapperError::EventWriteError(e.to_string()))?;
        Ok(())
    }

    /// Write a sync event
    pub fn sync(&self) -> Result<()> {
        let sync_event = InputEvent::sync();
        self.write_event(&sync_event)
    }

    /// Write a key press event
    pub fn key_press(&self, code: u16) -> Result<()> {
        self.write_event(&InputEvent::key_press(code))?;
        self.sync()
    }

    /// Write a key release event
    pub fn key_release(&self, code: u16) -> Result<()> {
        self.write_event(&InputEvent::key_release(code))?;
        self.sync()
    }

    /// Write a key tap (press + release)
    pub fn key_tap(&self, code: u16) -> Result<()> {
        self.write_event(&InputEvent::key_press(code))?;
        self.write_event(&InputEvent::key_release(code))?;
        self.sync()
    }
}
