//! Windows input device implementation using Raw Input API for keyboard/mouse
//! and gilrs for gamepads
//!
//! # Keyboard/Mouse Input Capture
//!
//! This implementation uses the Windows Raw Input API to:
//! - Enumerate HID devices (keyboards, mice)
//! - Open devices and read input reports
//! - Translate Raw Input events to platform-agnostic events
//! - Support capturing from specific devices (not just global)
//!
//! # Gamepad Input
//!
//! Gamepad input continues to use the gilrs library which handles
//! XInput and DirectInput integration.
//!
//! # Permissions
//!
//! - Keyboard/Mouse: No special permissions needed, but some applications
//!   (games with anti-cheat) may block Raw Input hooking
//! - Low-level hooks may require running with elevated privileges in some scenarios
//! - Gamepad: No special permissions needed

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use gilrs::{ev::Axis, ev::Button, Event, EventType, Gilrs};
use tokio::sync::mpsc;
use tracing::{debug, error, trace, warn};

use crate::core::error::{RemapperError, Result};
use crate::platform::traits::{
    AbsAxisInfo, DeviceCapabilities, DeviceType, InputBackend, PlatformDeviceInfo,
    PlatformInputDevice, PlatformInputEvent,
};

#[cfg(windows)]
use windows::{
    core::PCWSTR,
    Win32::{
        Devices::HumanInterfaceDevice::{
            HidD_GetHidGuid, HidD_GetProductString, HidD_GetManufacturerString,
            HIDD_ATTRIBUTES, HidD_GetAttributes,
        },
        Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM, GetLastError, CloseHandle},
        Graphics::Gdi::HBRUSH,
        System::LibraryLoader::GetModuleHandleW,
        UI::Input::{
            GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList,
            RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
            RAWINPUTDEVICELIST, RAWINPUTHEADER, RID_DEVICE_INFO, RID_DEVICE_INFO_TYPE,
            RID_INPUT, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK,
            RIDI_DEVICEINFO, RIDI_DEVICENAME,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
            GetMessageW, PeekMessageW, PostQuitMessage, RegisterClassW,
            TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HMENU, HICON,
            HCURSOR, MSG, PM_REMOVE, WM_DESTROY, WM_INPUT, WM_INPUT_DEVICE_CHANGE,
            WNDCLASSW, WS_OVERLAPPEDWINDOW, WINDOW_EX_STYLE,
        },
    },
};

/// Windows input backend using Raw Input API for keyboard/mouse and gilrs for gamepads
pub struct WindowsInputBackend {
    gilrs: Arc<Mutex<Gilrs>>,
    raw_input_devices: Arc<RwLock<Vec<RawInputDeviceInfo>>>,
}

/// Information about a Raw Input device
#[derive(Debug, Clone)]
struct RawInputDeviceInfo {
    /// Device handle
    handle: isize,
    /// Human-readable name
    name: String,
    /// Vendor ID
    vendor_id: u16,
    /// Product ID
    product_id: u16,
    /// Device type
    device_type: DeviceType,
    /// Device path
    path: String,
}

impl WindowsInputBackend {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().expect("Failed to initialize gilrs");
        let raw_input_devices = Arc::new(RwLock::new(Vec::new()));

        let backend = Self {
            gilrs: Arc::new(Mutex::new(gilrs)),
            raw_input_devices,
        };

        // Enumerate Raw Input devices on creation
        if let Err(e) = backend.enumerate_raw_input_devices() {
            warn!("Failed to enumerate Raw Input devices: {}", e);
        }

        backend
    }

    /// Enumerate all Raw Input devices (keyboards and mice)
    #[cfg(windows)]
    fn enumerate_raw_input_devices(&self) -> Result<()> {
        let mut device_count: u32 = 0;

        // Get the number of devices
        unsafe {
            let result = GetRawInputDeviceList(
                None,
                &mut device_count,
                size_of::<RAWINPUTDEVICELIST>() as u32,
            );
            if result == u32::MAX {
                return Err(RemapperError::DeviceNotFound(
                    "Failed to get Raw Input device count".to_string(),
                ));
            }
        }

        if device_count == 0 {
            debug!("No Raw Input devices found");
            return Ok(());
        }

        debug!("Found {} Raw Input devices", device_count);

        // Allocate buffer for device list
        let mut device_list: Vec<RAWINPUTDEVICELIST> =
            vec![RAWINPUTDEVICELIST::default(); device_count as usize];

        // Get the device list
        unsafe {
            let result = GetRawInputDeviceList(
                Some(device_list.as_mut_ptr()),
                &mut device_count,
                size_of::<RAWINPUTDEVICELIST>() as u32,
            );
            if result == u32::MAX {
                return Err(RemapperError::DeviceNotFound(
                    "Failed to get Raw Input device list".to_string(),
                ));
            }
        }

        let mut devices = Vec::new();

        for device in &device_list {
            // Get device info
            let mut device_info = RID_DEVICE_INFO::default();
            device_info.cbSize = size_of::<RID_DEVICE_INFO>() as u32;
            let mut size = size_of::<RID_DEVICE_INFO>() as u32;

            let result = unsafe {
                GetRawInputDeviceInfoW(
                    device.hDevice,
                    RIDI_DEVICEINFO,
                    Some(&mut device_info as *mut _ as *mut c_void),
                    &mut size,
                )
            };

            if result == u32::MAX {
                continue;
            }

            // Determine device type
            let device_type = match RID_DEVICE_INFO_TYPE(device_info.dwType) {
                RID_DEVICE_INFO_TYPE(0) => DeviceType::Mouse,    // RIM_TYPEMOUSE
                RID_DEVICE_INFO_TYPE(1) => DeviceType::Keyboard, // RIM_TYPEKEYBOARD
                _ => continue, // Skip HID and unknown devices (gamepads handled by gilrs)
            };

            // Get device name (path)
            let mut name_size: u32 = 0;
            unsafe {
                GetRawInputDeviceInfoW(device.hDevice, RIDI_DEVICENAME, None, &mut name_size);
            }

            if name_size == 0 {
                continue;
            }

            let mut name_buf: Vec<u16> = vec![0; name_size as usize];
            let result = unsafe {
                GetRawInputDeviceInfoW(
                    device.hDevice,
                    RIDI_DEVICENAME,
                    Some(name_buf.as_mut_ptr() as *mut c_void),
                    &mut name_size,
                )
            };

            if result == u32::MAX {
                continue;
            }

            let device_path = String::from_utf16_lossy(&name_buf[..name_size as usize])
                .trim_end_matches('\0')
                .to_string();

            // Extract vendor/product IDs from device path or device info
            let (vendor_id, product_id) = match RID_DEVICE_INFO_TYPE(device_info.dwType) {
                RID_DEVICE_INFO_TYPE(0) => unsafe {
                    // Mouse
                    (
                        device_info.Anonymous.mouse.dwId as u16,
                        device_info.Anonymous.mouse.dwNumberOfButtons as u16,
                    )
                },
                RID_DEVICE_INFO_TYPE(1) => unsafe {
                    // Keyboard
                    (
                        device_info.Anonymous.keyboard.dwType as u16,
                        device_info.Anonymous.keyboard.dwSubType as u16,
                    )
                },
                _ => (0, 0),
            };

            // Try to extract VID/PID from device path (format: \\?\HID#VID_xxxx&PID_xxxx...)
            let (vid, pid) = parse_vid_pid_from_path(&device_path).unwrap_or((vendor_id, product_id));

            // Generate a human-readable name
            let friendly_name = generate_device_name(&device_path, device_type, vid, pid);

            let raw_device_info = RawInputDeviceInfo {
                handle: device.hDevice.0 as isize,
                name: friendly_name,
                vendor_id: vid,
                product_id: pid,
                device_type,
                path: device_path,
            };

            debug!(
                "Found Raw Input device: {} (VID:{:04X} PID:{:04X}) - {:?}",
                raw_device_info.name, raw_device_info.vendor_id, raw_device_info.product_id, device_type
            );

            devices.push(raw_device_info);
        }

        // Store the devices
        let mut raw_devices = self.raw_input_devices.write().map_err(|_| {
            RemapperError::DeviceNotFound("Failed to lock Raw Input devices".to_string())
        })?;
        *raw_devices = devices;

        Ok(())
    }

    #[cfg(not(windows))]
    fn enumerate_raw_input_devices(&self) -> Result<()> {
        Ok(())
    }
}

impl Default for WindowsInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse VID and PID from a Windows device path
fn parse_vid_pid_from_path(path: &str) -> Option<(u16, u16)> {
    // Device path format: \\?\HID#VID_xxxx&PID_xxxx#...
    let path_upper = path.to_uppercase();

    let vid_start = path_upper.find("VID_")?;
    let vid_str = &path_upper[vid_start + 4..vid_start + 8];
    let vid = u16::from_str_radix(vid_str, 16).ok()?;

    let pid_start = path_upper.find("PID_")?;
    let pid_str = &path_upper[pid_start + 4..pid_start + 8];
    let pid = u16::from_str_radix(pid_str, 16).ok()?;

    Some((vid, pid))
}

/// Generate a human-readable device name
fn generate_device_name(path: &str, device_type: DeviceType, vid: u16, pid: u16) -> String {
    let type_str = match device_type {
        DeviceType::Keyboard => "Keyboard",
        DeviceType::Mouse => "Mouse",
        _ => "Device",
    };

    // Try to identify common devices by VID/PID
    let device_name = match (vid, pid) {
        // Microsoft devices
        (0x045E, _) => "Microsoft".to_string(),
        // Logitech devices
        (0x046D, _) => "Logitech".to_string(),
        // Razer devices
        (0x1532, _) => "Razer".to_string(),
        // Corsair devices
        (0x1B1C, _) => "Corsair".to_string(),
        // SteelSeries devices
        (0x1038, _) => "SteelSeries".to_string(),
        // HyperX devices
        (0x0951, _) => "HyperX".to_string(),
        // Generic
        _ if vid == 0 && pid == 0 => "Generic".to_string(),
        _ => format!("{:04X}:{:04X}", vid, pid),
    };

    format!("{} {}", device_name, type_str)
}

#[async_trait]
impl InputBackend for WindowsInputBackend {
    async fn list_devices(&self) -> Result<Vec<PlatformDeviceInfo>> {
        let mut devices = Vec::new();

        // List gamepads from gilrs
        {
            let gilrs = self.gilrs.lock().map_err(|_| {
                RemapperError::DeviceNotFound("Failed to lock gilrs".to_string())
            })?;

            for (id, gamepad) in gilrs.gamepads() {
                let info = PlatformDeviceInfo {
                    id: format!("gamepad:{}", usize::from(id)),
                    name: gamepad.name().to_string(),
                    vendor_id: gamepad.vendor_id().unwrap_or(0),
                    product_id: gamepad.product_id().unwrap_or(0),
                    device_type: DeviceType::Gamepad,
                    path: None,
                    supports_grab: false,
                };
                debug!("Found gamepad: {} ({})", info.name, info.id);
                devices.push(info);
            }
        }

        // Refresh Raw Input device list
        if let Err(e) = self.enumerate_raw_input_devices() {
            warn!("Failed to refresh Raw Input devices: {}", e);
        }

        // List keyboards and mice from Raw Input
        {
            let raw_devices = self.raw_input_devices.read().map_err(|_| {
                RemapperError::DeviceNotFound("Failed to lock Raw Input devices".to_string())
            })?;

            for device in raw_devices.iter() {
                let prefix = match device.device_type {
                    DeviceType::Keyboard => "keyboard",
                    DeviceType::Mouse => "mouse",
                    _ => continue,
                };

                let info = PlatformDeviceInfo {
                    id: format!("{}:{:X}", prefix, device.handle),
                    name: device.name.clone(),
                    vendor_id: device.vendor_id,
                    product_id: device.product_id,
                    device_type: device.device_type,
                    path: Some(device.path.clone().into()),
                    supports_grab: false, // Windows doesn't support true device grabbing
                };
                debug!("Found Raw Input device: {} ({})", info.name, info.id);
                devices.push(info);
            }
        }

        Ok(devices)
    }

    async fn open_device(&self, device_id: &str) -> Result<Box<dyn PlatformInputDevice>> {
        if device_id.starts_with("gamepad:") {
            let id_str = device_id.strip_prefix("gamepad:").unwrap_or("0");
            let id: usize = id_str.parse().map_err(|_| {
                RemapperError::DeviceNotFound(format!("Invalid gamepad ID: {}", device_id))
            })?;

            let device = WindowsGamepadDevice::open(self.gilrs.clone(), id)?;
            Ok(Box::new(device))
        } else if device_id.starts_with("keyboard:") || device_id.starts_with("mouse:") {
            let (prefix, handle_str) = if device_id.starts_with("keyboard:") {
                ("keyboard:", device_id.strip_prefix("keyboard:").unwrap_or("0"))
            } else {
                ("mouse:", device_id.strip_prefix("mouse:").unwrap_or("0"))
            };

            let handle = isize::from_str_radix(handle_str, 16).map_err(|_| {
                RemapperError::DeviceNotFound(format!("Invalid device handle: {}", device_id))
            })?;

            // Find the device info
            let raw_devices = self.raw_input_devices.read().map_err(|_| {
                RemapperError::DeviceNotFound("Failed to lock Raw Input devices".to_string())
            })?;

            let device_info = raw_devices
                .iter()
                .find(|d| d.handle == handle)
                .ok_or_else(|| {
                    RemapperError::DeviceNotFound(format!("Device not found: {}", device_id))
                })?
                .clone();

            drop(raw_devices);

            let device = WindowsRawInputDevice::open(device_info)?;
            Ok(Box::new(device))
        } else {
            Err(RemapperError::DeviceNotFound(format!(
                "Unknown device type: {}",
                device_id
            )))
        }
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<PlatformDeviceInfo>> {
        let devices = self.list_devices().await?;
        let name_lower = name.to_lowercase();
        Ok(devices
            .into_iter()
            .find(|d| d.name.to_lowercase().contains(&name_lower)))
    }

    async fn find_by_id(&self, vendor: u16, product: u16) -> Result<Option<PlatformDeviceInfo>> {
        let devices = self.list_devices().await?;
        Ok(devices
            .into_iter()
            .find(|d| d.vendor_id == vendor && d.product_id == product))
    }
}

/// Windows gamepad input device using gilrs
pub struct WindowsGamepadDevice {
    gilrs: Arc<Mutex<Gilrs>>,
    gamepad_id: gilrs::GamepadId,
    info: PlatformDeviceInfo,
    grabbed: bool,
}

impl WindowsGamepadDevice {
    /// Open a gamepad device by index
    pub fn open(gilrs: Arc<Mutex<Gilrs>>, index: usize) -> Result<Self> {
        let gilrs_guard = gilrs.lock().map_err(|_| {
            RemapperError::DeviceNotFound("Failed to lock gilrs".to_string())
        })?;

        let (gamepad_id, gamepad) = gilrs_guard
            .gamepads()
            .nth(index)
            .ok_or_else(|| RemapperError::DeviceNotFound(format!("Gamepad {} not found", index)))?;

        let info = PlatformDeviceInfo {
            id: format!("gamepad:{}", usize::from(gamepad_id)),
            name: gamepad.name().to_string(),
            vendor_id: gamepad.vendor_id().unwrap_or(0),
            product_id: gamepad.product_id().unwrap_or(0),
            device_type: DeviceType::Gamepad,
            path: None,
            supports_grab: false,
        };

        debug!("Opened gamepad: {} ({})", info.name, info.id);

        drop(gilrs_guard);

        Ok(Self {
            gilrs,
            gamepad_id,
            info,
            grabbed: false,
        })
    }
}

#[async_trait]
impl PlatformInputDevice for WindowsGamepadDevice {
    async fn read_event(&mut self) -> Result<Option<PlatformInputEvent>> {
        // Poll for events with a small sleep to prevent busy-waiting
        tokio::time::sleep(Duration::from_millis(1)).await;

        let mut gilrs = self.gilrs.lock().map_err(|_| {
            RemapperError::EventReadError("Failed to lock gilrs".to_string())
        })?;

        // Process next event
        while let Some(event) = gilrs.next_event() {
            if event.id != self.gamepad_id {
                continue;
            }

            if let Some(platform_event) = gilrs_event_to_platform(&event) {
                trace!("Read event: {:?}", platform_event);
                return Ok(Some(platform_event));
            }
        }

        Ok(None)
    }

    async fn grab(&mut self) -> Result<()> {
        if !self.grabbed {
            warn!(
                "Device grabbing is not fully supported on Windows. \
                Other applications may still receive input from this device."
            );
            self.grabbed = true;
        }
        Ok(())
    }

    async fn ungrab(&mut self) -> Result<()> {
        self.grabbed = false;
        Ok(())
    }

    fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    fn info(&self) -> &PlatformDeviceInfo {
        &self.info
    }

    fn capabilities(&self) -> DeviceCapabilities {
        // Return standard gamepad capabilities
        let keys = vec![
            // Standard gamepad buttons mapped to evdev-compatible codes
            304, 305, 306, 307, // BTN_SOUTH, EAST, C, NORTH (A, B, X, Y)
            308, 309,           // BTN_WEST, Z (LB, RB on Xbox)
            310, 311,           // BTN_TL, BTN_TR (LT, RT digital)
            312, 313,           // BTN_TL2, BTN_TR2
            314, 315,           // BTN_SELECT, BTN_START
            316, 317, 318,      // BTN_MODE, BTN_THUMBL, BTN_THUMBR
        ];

        let abs_axes = vec![
            AbsAxisInfo {
                code: 0,  // ABS_X - Left stick X
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 1,  // ABS_Y - Left stick Y
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 3,  // ABS_RX - Right stick X
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 4,  // ABS_RY - Right stick Y
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 2,  // ABS_Z - Left trigger
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 5,  // ABS_RZ - Right trigger
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 16, // ABS_HAT0X - D-pad X
                value: 0,
                minimum: -1,
                maximum: 1,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 17, // ABS_HAT0Y - D-pad Y
                value: 0,
                minimum: -1,
                maximum: 1,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        ];

        DeviceCapabilities {
            keys,
            abs_axes,
            rel_axes: vec![],
        }
    }
}

/// Windows Raw Input device for keyboard/mouse
pub struct WindowsRawInputDevice {
    device_info: RawInputDeviceInfo,
    info: PlatformDeviceInfo,
    grabbed: bool,
    event_receiver: mpsc::Receiver<PlatformInputEvent>,
    running: Arc<AtomicBool>,
    #[cfg(windows)]
    _window_thread: Option<thread::JoinHandle<()>>,
}

/// Context passed to the message window callback
#[cfg(windows)]
struct RawInputContext {
    device_handle: isize,
    device_type: DeviceType,
    event_sender: mpsc::Sender<PlatformInputEvent>,
    running: Arc<AtomicBool>,
}

impl WindowsRawInputDevice {
    /// Open a Raw Input device
    #[cfg(windows)]
    pub fn open(device_info: RawInputDeviceInfo) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::channel(1024);
        let running = Arc::new(AtomicBool::new(true));

        let info = PlatformDeviceInfo {
            id: format!(
                "{}:{:X}",
                if device_info.device_type == DeviceType::Keyboard {
                    "keyboard"
                } else {
                    "mouse"
                },
                device_info.handle
            ),
            name: device_info.name.clone(),
            vendor_id: device_info.vendor_id,
            product_id: device_info.product_id,
            device_type: device_info.device_type,
            path: Some(device_info.path.clone().into()),
            supports_grab: false,
        };

        // Start the message window thread for Raw Input
        let device_handle = device_info.handle;
        let device_type = device_info.device_type;
        let running_clone = running.clone();

        let window_thread = thread::spawn(move || {
            if let Err(e) =
                run_raw_input_message_loop(device_handle, device_type, event_sender, running_clone)
            {
                error!("Raw Input message loop error: {}", e);
            }
        });

        debug!(
            "Opened Raw Input device: {} (handle: {:X})",
            info.name, device_handle
        );

        Ok(Self {
            device_info,
            info,
            grabbed: false,
            event_receiver,
            running,
            _window_thread: Some(window_thread),
        })
    }

    #[cfg(not(windows))]
    pub fn open(_device_info: RawInputDeviceInfo) -> Result<Self> {
        Err(RemapperError::NotSupported(
            "Raw Input is only available on Windows".to_string(),
        ))
    }
}

impl Drop for WindowsRawInputDevice {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[async_trait]
impl PlatformInputDevice for WindowsRawInputDevice {
    async fn read_event(&mut self) -> Result<Option<PlatformInputEvent>> {
        // Try to receive an event with a timeout
        match tokio::time::timeout(Duration::from_millis(10), self.event_receiver.recv()).await {
            Ok(Some(event)) => {
                trace!("Read Raw Input event: {:?}", event);
                Ok(Some(event))
            }
            Ok(None) => {
                // Channel closed
                Err(RemapperError::EventReadError(
                    "Raw Input channel closed".to_string(),
                ))
            }
            Err(_) => {
                // Timeout - no event available
                Ok(None)
            }
        }
    }

    async fn grab(&mut self) -> Result<()> {
        if !self.grabbed {
            warn!(
                "Device grabbing is not fully supported on Windows. \
                Other applications may still receive input from this device."
            );
            self.grabbed = true;
        }
        Ok(())
    }

    async fn ungrab(&mut self) -> Result<()> {
        self.grabbed = false;
        Ok(())
    }

    fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    fn info(&self) -> &PlatformDeviceInfo {
        &self.info
    }

    fn capabilities(&self) -> DeviceCapabilities {
        match self.device_info.device_type {
            DeviceType::Keyboard => {
                // Standard keyboard capabilities
                let keys: Vec<u16> = (1..=88)
                    .chain(96..=111)
                    .chain(vec![119, 125, 126])
                    .collect();

                DeviceCapabilities {
                    keys,
                    abs_axes: vec![],
                    rel_axes: vec![],
                }
            }
            DeviceType::Mouse => {
                // Standard mouse capabilities
                let keys = vec![272, 273, 274, 275, 276]; // BTN_LEFT, RIGHT, MIDDLE, SIDE, EXTRA
                let rel_axes = vec![0, 1, 8, 6]; // REL_X, REL_Y, REL_WHEEL, REL_HWHEEL

                DeviceCapabilities {
                    keys,
                    abs_axes: vec![],
                    rel_axes,
                }
            }
            _ => DeviceCapabilities::default(),
        }
    }
}

/// Run the Raw Input message loop in a separate thread
#[cfg(windows)]
fn run_raw_input_message_loop(
    device_handle: isize,
    device_type: DeviceType,
    event_sender: mpsc::Sender<PlatformInputEvent>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    // Create a hidden message window
    let hwnd = create_message_window()?;

    // Register for Raw Input
    register_raw_input_devices(hwnd, device_type)?;

    debug!(
        "Raw Input message loop started for device {:X} (type: {:?})",
        device_handle, device_type
    );

    // Store context in thread-local storage for the callback
    THREAD_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = Some(RawInputContext {
            device_handle,
            device_type,
            event_sender,
            running: running.clone(),
        });
    });

    // Message loop
    let mut msg = MSG::default();
    while running.load(Ordering::SeqCst) {
        unsafe {
            // Use PeekMessage with a timeout to allow checking the running flag
            if PeekMessageW(&mut msg, Some(hwnd), 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_DESTROY {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                // Sleep briefly to prevent busy-waiting
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    // Cleanup
    unsafe {
        DestroyWindow(hwnd);
    }

    debug!("Raw Input message loop ended for device {:X}", device_handle);

    Ok(())
}

// Thread-local storage for Raw Input context
#[cfg(windows)]
thread_local! {
    static THREAD_CONTEXT: std::cell::RefCell<Option<RawInputContext>> = const { std::cell::RefCell::new(None) };
}

/// Create a hidden message window for Raw Input
#[cfg(windows)]
fn create_message_window() -> Result<HWND> {
    use std::sync::atomic::AtomicUsize;

    static WINDOW_CLASS_COUNTER: AtomicUsize = AtomicUsize::new(0);

    let class_name_str = format!(
        "RemapperRawInput{}",
        WINDOW_CLASS_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let class_name: Vec<u16> = class_name_str.encode_utf16().chain(std::iter::once(0)).collect();

    let hinstance = unsafe { GetModuleHandleW(PCWSTR::null())? };

    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(raw_input_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance.into(),
        hIcon: HICON::default(),
        hCursor: HCURSOR::default(),
        hbrBackground: HBRUSH::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
    };

    let atom = unsafe { RegisterClassW(&wnd_class) };
    if atom == 0 {
        return Err(RemapperError::DeviceNotFound(
            "Failed to register window class".to_string(),
        ));
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND::default(),
            HMENU::default(),
            Some(hinstance.into()),
            None,
        )?
    };

    Ok(hwnd)
}

/// Register for Raw Input devices
#[cfg(windows)]
fn register_raw_input_devices(hwnd: HWND, device_type: DeviceType) -> Result<()> {
    // HID usage page and usage codes
    const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
    const HID_USAGE_GENERIC_MOUSE: u16 = 0x02;
    const HID_USAGE_GENERIC_KEYBOARD: u16 = 0x06;

    let usage = match device_type {
        DeviceType::Keyboard => HID_USAGE_GENERIC_KEYBOARD,
        DeviceType::Mouse => HID_USAGE_GENERIC_MOUSE,
        _ => {
            return Err(RemapperError::NotSupported(
                "Unsupported device type for Raw Input".to_string(),
            ));
        }
    };

    let raw_input_device = RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: usage,
        dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
        hwndTarget: hwnd,
    };

    let result = unsafe {
        RegisterRawInputDevices(&[raw_input_device], size_of::<RAWINPUTDEVICE>() as u32)
    };

    if !result.as_bool() {
        return Err(RemapperError::DeviceNotFound(format!(
            "Failed to register Raw Input device: {:?}",
            unsafe { GetLastError() }
        )));
    }

    debug!(
        "Registered for Raw Input: usage page {:04X}, usage {:04X}",
        HID_USAGE_PAGE_GENERIC, usage
    );

    Ok(())
}

/// Window procedure for Raw Input messages
#[cfg(windows)]
unsafe extern "system" fn raw_input_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_INPUT => {
            handle_raw_input(lparam);
            LRESULT(0)
        }
        WM_INPUT_DEVICE_CHANGE => {
            // Device added or removed - could refresh device list here
            debug!("Raw Input device change detected");
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Handle a WM_INPUT message
#[cfg(windows)]
fn handle_raw_input(lparam: LPARAM) {
    // Get the size of the raw input data
    let mut size: u32 = 0;
    let result = unsafe {
        GetRawInputData(
            HRAWINPUT(lparam.0 as *mut c_void),
            RID_INPUT,
            None,
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };

    if result == u32::MAX || size == 0 {
        return;
    }

    // Allocate buffer and get the data
    let mut buffer: Vec<u8> = vec![0; size as usize];
    let result = unsafe {
        GetRawInputData(
            HRAWINPUT(lparam.0 as *mut c_void),
            RID_INPUT,
            Some(buffer.as_mut_ptr() as *mut c_void),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };

    if result == u32::MAX {
        return;
    }

    // Parse the raw input
    let raw_input = unsafe { &*(buffer.as_ptr() as *const RAWINPUT) };
    let device_handle = raw_input.header.hDevice.0 as isize;

    // Get the context from thread-local storage
    THREAD_CONTEXT.with(|ctx| {
        if let Some(ref context) = *ctx.borrow() {
            // Check if this event is from our target device
            // If device_handle is 0, we accept all events (for global capture)
            // Otherwise, only process events from our specific device
            if context.device_handle != 0 && device_handle != context.device_handle {
                return;
            }

            // Convert to platform events based on device type
            let events = match RID_DEVICE_INFO_TYPE(raw_input.header.dwType) {
                RID_DEVICE_INFO_TYPE(0) => {
                    // Mouse
                    convert_raw_mouse_input(raw_input)
                }
                RID_DEVICE_INFO_TYPE(1) => {
                    // Keyboard
                    convert_raw_keyboard_input(raw_input)
                }
                _ => vec![],
            };

            // Send events
            for event in events {
                if let Err(e) = context.event_sender.blocking_send(event) {
                    trace!("Failed to send Raw Input event: {}", e);
                }
            }
        }
    });
}

/// Convert raw keyboard input to platform events
#[cfg(windows)]
fn convert_raw_keyboard_input(raw_input: &RAWINPUT) -> Vec<PlatformInputEvent> {
    let keyboard = unsafe { &raw_input.data.keyboard };
    let mut events = Vec::new();

    // Get the virtual key and scan code
    let vkey = keyboard.VKey;
    let scan_code = keyboard.MakeCode;
    let flags = keyboard.Flags;

    // Determine if key is pressed or released
    // RI_KEY_BREAK (bit 0) indicates key release
    let is_release = (flags & 0x01) != 0;
    let value = if is_release { 0 } else { 1 };

    // Convert Windows virtual key to evdev keycode
    let evdev_code = windows_vkey_to_evdev(vkey);

    if evdev_code != 0 {
        events.push(PlatformInputEvent::new(1, evdev_code, value)); // EV_KEY
        events.push(PlatformInputEvent::sync());
    }

    trace!(
        "Keyboard: VKey={:04X}, ScanCode={:04X}, Flags={:04X} -> evdev={}, value={}",
        vkey,
        scan_code,
        flags,
        evdev_code,
        value
    );

    events
}

/// Convert raw mouse input to platform events
#[cfg(windows)]
fn convert_raw_mouse_input(raw_input: &RAWINPUT) -> Vec<PlatformInputEvent> {
    let mouse = unsafe { &raw_input.data.mouse };
    let mut events = Vec::new();

    // Handle relative movement
    // Check if this is relative movement (not absolute)
    let flags = mouse.usFlags;
    let is_absolute = (flags & 0x01) != 0; // MOUSE_MOVE_ABSOLUTE

    if !is_absolute {
        let dx = mouse.lLastX;
        let dy = mouse.lLastY;

        if dx != 0 {
            events.push(PlatformInputEvent::new(2, 0, dx)); // EV_REL, REL_X
        }
        if dy != 0 {
            events.push(PlatformInputEvent::new(2, 1, dy)); // EV_REL, REL_Y
        }
    }

    // Handle button events
    let button_flags = mouse.Anonymous.Anonymous.usButtonFlags;

    // Left button
    if (button_flags & 0x0001) != 0 {
        // RI_MOUSE_LEFT_BUTTON_DOWN
        events.push(PlatformInputEvent::new(1, 272, 1)); // EV_KEY, BTN_LEFT, press
    }
    if (button_flags & 0x0002) != 0 {
        // RI_MOUSE_LEFT_BUTTON_UP
        events.push(PlatformInputEvent::new(1, 272, 0)); // EV_KEY, BTN_LEFT, release
    }

    // Right button
    if (button_flags & 0x0004) != 0 {
        // RI_MOUSE_RIGHT_BUTTON_DOWN
        events.push(PlatformInputEvent::new(1, 273, 1)); // EV_KEY, BTN_RIGHT, press
    }
    if (button_flags & 0x0008) != 0 {
        // RI_MOUSE_RIGHT_BUTTON_UP
        events.push(PlatformInputEvent::new(1, 273, 0)); // EV_KEY, BTN_RIGHT, release
    }

    // Middle button
    if (button_flags & 0x0010) != 0 {
        // RI_MOUSE_MIDDLE_BUTTON_DOWN
        events.push(PlatformInputEvent::new(1, 274, 1)); // EV_KEY, BTN_MIDDLE, press
    }
    if (button_flags & 0x0020) != 0 {
        // RI_MOUSE_MIDDLE_BUTTON_UP
        events.push(PlatformInputEvent::new(1, 274, 0)); // EV_KEY, BTN_MIDDLE, release
    }

    // Side buttons (Button 4 and 5)
    if (button_flags & 0x0040) != 0 {
        // RI_MOUSE_BUTTON_4_DOWN
        events.push(PlatformInputEvent::new(1, 275, 1)); // EV_KEY, BTN_SIDE, press
    }
    if (button_flags & 0x0080) != 0 {
        // RI_MOUSE_BUTTON_4_UP
        events.push(PlatformInputEvent::new(1, 275, 0)); // EV_KEY, BTN_SIDE, release
    }
    if (button_flags & 0x0100) != 0 {
        // RI_MOUSE_BUTTON_5_DOWN
        events.push(PlatformInputEvent::new(1, 276, 1)); // EV_KEY, BTN_EXTRA, press
    }
    if (button_flags & 0x0200) != 0 {
        // RI_MOUSE_BUTTON_5_UP
        events.push(PlatformInputEvent::new(1, 276, 0)); // EV_KEY, BTN_EXTRA, release
    }

    // Handle wheel
    if (button_flags & 0x0400) != 0 {
        // RI_MOUSE_WHEEL
        let wheel_delta = mouse.Anonymous.Anonymous.usButtonData as i16 as i32;
        // Normalize to evdev convention (positive = up/forward)
        let wheel_value = wheel_delta / 120; // Windows uses 120 units per notch
        if wheel_value != 0 {
            events.push(PlatformInputEvent::new(2, 8, wheel_value)); // EV_REL, REL_WHEEL
        }
    }

    // Handle horizontal wheel
    if (button_flags & 0x0800) != 0 {
        // RI_MOUSE_HWHEEL
        let wheel_delta = mouse.Anonymous.Anonymous.usButtonData as i16 as i32;
        let wheel_value = wheel_delta / 120;
        if wheel_value != 0 {
            events.push(PlatformInputEvent::new(2, 6, wheel_value)); // EV_REL, REL_HWHEEL
        }
    }

    // Add sync event if we generated any events
    if !events.is_empty() {
        events.push(PlatformInputEvent::sync());
    }

    trace!(
        "Mouse: Flags={:04X}, ButtonFlags={:04X}, dX={}, dY={} -> {} events",
        flags,
        button_flags,
        mouse.lLastX,
        mouse.lLastY,
        events.len()
    );

    events
}

/// Convert Windows virtual key code to evdev keycode
#[cfg(windows)]
fn windows_vkey_to_evdev(vkey: u16) -> u16 {
    // Virtual key to evdev keycode mapping
    // Based on Windows VK_* codes to Linux KEY_* codes
    match vkey {
        // Letters A-Z (VK_A = 0x41 to VK_Z = 0x5A)
        0x41 => 30,  // KEY_A
        0x42 => 48,  // KEY_B
        0x43 => 46,  // KEY_C
        0x44 => 32,  // KEY_D
        0x45 => 18,  // KEY_E
        0x46 => 33,  // KEY_F
        0x47 => 34,  // KEY_G
        0x48 => 35,  // KEY_H
        0x49 => 23,  // KEY_I
        0x4A => 36,  // KEY_J
        0x4B => 37,  // KEY_K
        0x4C => 38,  // KEY_L
        0x4D => 50,  // KEY_M
        0x4E => 49,  // KEY_N
        0x4F => 24,  // KEY_O
        0x50 => 25,  // KEY_P
        0x51 => 16,  // KEY_Q
        0x52 => 19,  // KEY_R
        0x53 => 31,  // KEY_S
        0x54 => 20,  // KEY_T
        0x55 => 22,  // KEY_U
        0x56 => 47,  // KEY_V
        0x57 => 17,  // KEY_W
        0x58 => 45,  // KEY_X
        0x59 => 21,  // KEY_Y
        0x5A => 44,  // KEY_Z

        // Numbers 0-9 (VK_0 = 0x30 to VK_9 = 0x39)
        0x30 => 11,  // KEY_0
        0x31 => 2,   // KEY_1
        0x32 => 3,   // KEY_2
        0x33 => 4,   // KEY_3
        0x34 => 5,   // KEY_4
        0x35 => 6,   // KEY_5
        0x36 => 7,   // KEY_6
        0x37 => 8,   // KEY_7
        0x38 => 9,   // KEY_8
        0x39 => 10,  // KEY_9

        // Function keys
        0x70 => 59,  // VK_F1 -> KEY_F1
        0x71 => 60,  // VK_F2 -> KEY_F2
        0x72 => 61,  // VK_F3 -> KEY_F3
        0x73 => 62,  // VK_F4 -> KEY_F4
        0x74 => 63,  // VK_F5 -> KEY_F5
        0x75 => 64,  // VK_F6 -> KEY_F6
        0x76 => 65,  // VK_F7 -> KEY_F7
        0x77 => 66,  // VK_F8 -> KEY_F8
        0x78 => 67,  // VK_F9 -> KEY_F9
        0x79 => 68,  // VK_F10 -> KEY_F10
        0x7A => 87,  // VK_F11 -> KEY_F11
        0x7B => 88,  // VK_F12 -> KEY_F12

        // Numpad
        0x60 => 82,  // VK_NUMPAD0 -> KEY_KP0
        0x61 => 79,  // VK_NUMPAD1 -> KEY_KP1
        0x62 => 80,  // VK_NUMPAD2 -> KEY_KP2
        0x63 => 81,  // VK_NUMPAD3 -> KEY_KP3
        0x64 => 75,  // VK_NUMPAD4 -> KEY_KP4
        0x65 => 76,  // VK_NUMPAD5 -> KEY_KP5
        0x66 => 77,  // VK_NUMPAD6 -> KEY_KP6
        0x67 => 71,  // VK_NUMPAD7 -> KEY_KP7
        0x68 => 72,  // VK_NUMPAD8 -> KEY_KP8
        0x69 => 73,  // VK_NUMPAD9 -> KEY_KP9
        0x6A => 55,  // VK_MULTIPLY -> KEY_KPASTERISK
        0x6B => 78,  // VK_ADD -> KEY_KPPLUS
        0x6D => 74,  // VK_SUBTRACT -> KEY_KPMINUS
        0x6E => 83,  // VK_DECIMAL -> KEY_KPDOT
        0x6F => 98,  // VK_DIVIDE -> KEY_KPSLASH

        // Control keys
        0x08 => 14,  // VK_BACK -> KEY_BACKSPACE
        0x09 => 15,  // VK_TAB -> KEY_TAB
        0x0D => 28,  // VK_RETURN -> KEY_ENTER
        0x10 => 42,  // VK_SHIFT -> KEY_LEFTSHIFT
        0x11 => 29,  // VK_CONTROL -> KEY_LEFTCTRL
        0x12 => 56,  // VK_MENU (Alt) -> KEY_LEFTALT
        0x13 => 119, // VK_PAUSE -> KEY_PAUSE
        0x14 => 58,  // VK_CAPITAL -> KEY_CAPSLOCK
        0x1B => 1,   // VK_ESCAPE -> KEY_ESC
        0x20 => 57,  // VK_SPACE -> KEY_SPACE

        // Navigation keys
        0x21 => 104, // VK_PRIOR (Page Up) -> KEY_PAGEUP
        0x22 => 109, // VK_NEXT (Page Down) -> KEY_PAGEDOWN
        0x23 => 107, // VK_END -> KEY_END
        0x24 => 102, // VK_HOME -> KEY_HOME
        0x25 => 105, // VK_LEFT -> KEY_LEFT
        0x26 => 103, // VK_UP -> KEY_UP
        0x27 => 106, // VK_RIGHT -> KEY_RIGHT
        0x28 => 108, // VK_DOWN -> KEY_DOWN
        0x2D => 110, // VK_INSERT -> KEY_INSERT
        0x2E => 111, // VK_DELETE -> KEY_DELETE

        // Modifier keys
        0xA0 => 42,  // VK_LSHIFT -> KEY_LEFTSHIFT
        0xA1 => 54,  // VK_RSHIFT -> KEY_RIGHTSHIFT
        0xA2 => 29,  // VK_LCONTROL -> KEY_LEFTCTRL
        0xA3 => 97,  // VK_RCONTROL -> KEY_RIGHTCTRL
        0xA4 => 56,  // VK_LMENU (Left Alt) -> KEY_LEFTALT
        0xA5 => 100, // VK_RMENU (Right Alt) -> KEY_RIGHTALT

        // Windows keys
        0x5B => 125, // VK_LWIN -> KEY_LEFTMETA
        0x5C => 126, // VK_RWIN -> KEY_RIGHTMETA
        0x5D => 127, // VK_APPS (Menu) -> KEY_COMPOSE

        // Lock keys
        0x90 => 69,  // VK_NUMLOCK -> KEY_NUMLOCK
        0x91 => 70,  // VK_SCROLL -> KEY_SCROLLLOCK

        // Punctuation and symbols
        0xBA => 39,  // VK_OEM_1 (;:) -> KEY_SEMICOLON
        0xBB => 13,  // VK_OEM_PLUS (=+) -> KEY_EQUAL
        0xBC => 51,  // VK_OEM_COMMA (,<) -> KEY_COMMA
        0xBD => 12,  // VK_OEM_MINUS (-_) -> KEY_MINUS
        0xBE => 52,  // VK_OEM_PERIOD (.>) -> KEY_DOT
        0xBF => 53,  // VK_OEM_2 (/?) -> KEY_SLASH
        0xC0 => 41,  // VK_OEM_3 (`~) -> KEY_GRAVE
        0xDB => 26,  // VK_OEM_4 ([{) -> KEY_LEFTBRACE
        0xDC => 43,  // VK_OEM_5 (\|) -> KEY_BACKSLASH
        0xDD => 27,  // VK_OEM_6 (]}) -> KEY_RIGHTBRACE
        0xDE => 40,  // VK_OEM_7 ('") -> KEY_APOSTROPHE

        // Print Screen, etc.
        0x2C => 99,  // VK_SNAPSHOT -> KEY_SYSRQ (Print Screen)

        _ => 0, // Unknown key
    }
}

/// Convert gilrs event to platform event
fn gilrs_event_to_platform(event: &Event) -> Option<PlatformInputEvent> {
    match event.event {
        EventType::ButtonPressed(button, _) => {
            let code = button_to_code(button)?;
            Some(PlatformInputEvent::new(1, code, 1)) // EV_KEY, press
        }
        EventType::ButtonReleased(button, _) => {
            let code = button_to_code(button)?;
            Some(PlatformInputEvent::new(1, code, 0)) // EV_KEY, release
        }
        EventType::ButtonChanged(button, value, _) => {
            // For analog buttons like triggers
            let code = button_to_code(button)?;
            let int_value = (value * 255.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value)) // EV_ABS
        }
        EventType::AxisChanged(axis, value, _) => {
            let code = axis_to_code(axis)?;
            // Scale from -1.0..1.0 to -32768..32767
            let int_value = (value * 32767.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value)) // EV_ABS
        }
        EventType::Connected | EventType::Disconnected | EventType::Dropped => None,
        _ => None, // Handle any future EventType variants
    }
}

/// Convert gilrs Button to evdev-compatible code
fn button_to_code(button: Button) -> Option<u16> {
    match button {
        Button::South => Some(304),      // BTN_SOUTH / BTN_A
        Button::East => Some(305),       // BTN_EAST / BTN_B
        Button::North => Some(307),      // BTN_NORTH / BTN_X
        Button::West => Some(308),       // BTN_WEST / BTN_Y
        Button::LeftTrigger => Some(310),  // BTN_TL
        Button::RightTrigger => Some(311), // BTN_TR
        Button::LeftTrigger2 => Some(312), // BTN_TL2
        Button::RightTrigger2 => Some(313), // BTN_TR2
        Button::Select => Some(314),     // BTN_SELECT
        Button::Start => Some(315),      // BTN_START
        Button::Mode => Some(316),       // BTN_MODE
        Button::LeftThumb => Some(317),  // BTN_THUMBL
        Button::RightThumb => Some(318), // BTN_THUMBR
        Button::DPadUp => Some(0),       // Use HAT events instead
        Button::DPadDown => Some(0),
        Button::DPadLeft => Some(0),
        Button::DPadRight => Some(0),
        Button::C => Some(306),          // BTN_C
        Button::Z => Some(309),          // BTN_Z
        Button::Unknown => None,
    }
}

/// Convert gilrs Axis to evdev-compatible code
fn axis_to_code(axis: Axis) -> Option<u16> {
    match axis {
        Axis::LeftStickX => Some(0),   // ABS_X
        Axis::LeftStickY => Some(1),   // ABS_Y
        Axis::RightStickX => Some(3),  // ABS_RX
        Axis::RightStickY => Some(4),  // ABS_RY
        Axis::LeftZ => Some(2),        // ABS_Z (left trigger)
        Axis::RightZ => Some(5),       // ABS_RZ (right trigger)
        Axis::DPadX => Some(16),       // ABS_HAT0X
        Axis::DPadY => Some(17),       // ABS_HAT0Y
        Axis::Unknown => None,
    }
}

// Re-export types for module use
pub use WindowsInputBackend as WindowsInputBackend;
pub use WindowsRawInputDevice as WindowsInputDevice;
