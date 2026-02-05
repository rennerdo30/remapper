//! Linux virtual output device implementation using uinput

use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AbsInfo, AbsoluteAxisType, AttributeSet, Key, RelativeAxisType, UinputAbsSetup};
use std::sync::Mutex;
use tracing::{debug, trace};

use crate::core::error::{RemapperError, Result};
use crate::platform::traits::{
    DeviceCapabilities, DeviceType, OutputBackend, PlatformInputEvent, PlatformOutputDevice,
};

use super::input::u16_to_evdev_event_type;

/// Linux output backend using uinput
pub struct LinuxOutputBackend;

impl LinuxOutputBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxOutputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputBackend for LinuxOutputBackend {
    fn create_device(
        &self,
        name: &str,
        capabilities: &DeviceCapabilities,
    ) -> Result<Box<dyn PlatformOutputDevice>> {
        let device = LinuxOutputDevice::create(name, capabilities)?;
        Ok(Box::new(device))
    }

    fn supports_device_type(&self, _device_type: DeviceType) -> bool {
        // Linux uinput supports all device types
        true
    }

    fn is_available(&self) -> bool {
        // Check if /dev/uinput exists and is accessible
        std::fs::metadata("/dev/uinput").is_ok()
    }

    fn availability_message(&self) -> Option<String> {
        if self.is_available() {
            None
        } else {
            Some(
                "uinput module not loaded or /dev/uinput not accessible. \
                 Try: sudo modprobe uinput"
                    .to_string(),
            )
        }
    }
}

/// Linux virtual output device using uinput
pub struct LinuxOutputDevice {
    /// The uinput virtual device
    device: Mutex<VirtualDevice>,
    /// Device name
    name: String,
}

impl LinuxOutputDevice {
    /// Create a new virtual output device with specified capabilities
    pub fn create(name: &str, capabilities: &DeviceCapabilities) -> Result<Self> {
        let mut builder = VirtualDeviceBuilder::new()
            .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?
            .name(name);

        // Add supported keys
        if !capabilities.keys.is_empty() {
            let mut key_set = AttributeSet::<Key>::new();
            for key_code in &capabilities.keys {
                key_set.insert(Key::new(*key_code));
            }
            builder = builder
                .with_keys(&key_set)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
        }

        // Add supported relative axes
        if !capabilities.rel_axes.is_empty() {
            let mut rel_set = AttributeSet::<RelativeAxisType>::new();
            for axis_code in &capabilities.rel_axes {
                rel_set.insert(RelativeAxisType(*axis_code));
            }
            builder = builder
                .with_relative_axes(&rel_set)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
        }

        // Add supported absolute axes with their info
        for axis_info in &capabilities.abs_axes {
            let abs_setup = UinputAbsSetup::new(
                AbsoluteAxisType(axis_info.code),
                AbsInfo::new(
                    axis_info.value,
                    axis_info.minimum,
                    axis_info.maximum,
                    axis_info.fuzz,
                    axis_info.flat,
                    axis_info.resolution,
                ),
            );
            builder = builder
                .with_absolute_axis(&abs_setup)
                .map_err(|e| RemapperError::UInputCreationFailed(e.to_string()))?;
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
}

impl PlatformOutputDevice for LinuxOutputDevice {
    fn write_event(&self, event: &PlatformInputEvent) -> Result<()> {
        let event_type = u16_to_evdev_event_type(event.event_type)
            .ok_or_else(|| RemapperError::EventWriteError("Invalid event type".to_string()))?;

        let evdev_event = evdev::InputEvent::new(event_type, event.code, event.value);
        trace!("Writing event: {:?}", event);

        let mut device = self.device.lock().map_err(|_| {
            RemapperError::EventWriteError("Output device lock poisoned".to_string())
        })?;
        device
            .emit(&[evdev_event])
            .map_err(|e| RemapperError::EventWriteError(e.to_string()))?;
        Ok(())
    }

    fn write_events(&self, events: &[PlatformInputEvent]) -> Result<()> {
        let evdev_events: Vec<_> = events
            .iter()
            .filter_map(|e| {
                u16_to_evdev_event_type(e.event_type)
                    .map(|t| evdev::InputEvent::new(t, e.code, e.value))
            })
            .collect();

        let mut device = self.device.lock().map_err(|_| {
            RemapperError::EventWriteError("Output device lock poisoned".to_string())
        })?;
        device
            .emit(&evdev_events)
            .map_err(|e| RemapperError::EventWriteError(e.to_string()))?;
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        let sync_event = PlatformInputEvent::sync();
        self.write_event(&sync_event)
    }

    fn name(&self) -> &str {
        &self.name
    }
}
