//! macOS input device implementation using IOKit HID Manager for keyboard/mouse
//! and gilrs for gamepads
//!
//! # Keyboard/Mouse Input Capture
//!
//! macOS requires the "Input Monitoring" permission (System Settings > Privacy & Security
//! > Input Monitoring) to capture keyboard and mouse events from HID devices.
//!
//! This implementation uses IOKit HID Manager to:
//! - Enumerate HID devices (keyboards, mice, trackpads)
//! - Open devices and read input reports
//! - Translate HID events to platform-agnostic events
//!
//! # Gamepad Input
//!
//! Gamepad input continues to use the gilrs library which handles
//! the Game Controller framework integration.
//!
//! # Permissions
//!
//! - Keyboard/Mouse: Input Monitoring permission required
//! - Gamepad: No special permissions needed (gilrs uses Game Controller framework)

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use gilrs::{ev::Axis, ev::Button, Event, EventType, Gilrs};
use tracing::{debug, error, info, trace, warn};

use crate::core::error::{RemapperError, Result};
use crate::platform::traits::{
    AbsAxisInfo, DeviceCapabilities, DeviceType, InputBackend, PlatformDeviceInfo,
    PlatformInputDevice, PlatformInputEvent,
};

// IOKit HID Manager bindings
mod hid {
    use std::ffi::c_void;

    pub type IOHIDManagerRef = *mut c_void;
    pub type IOHIDDeviceRef = *mut c_void;
    pub type IOHIDValueRef = *mut c_void;
    pub type IOHIDElementRef = *mut c_void;
    pub type CFRunLoopRef = *mut c_void;
    pub type CFStringRef = *mut c_void;
    pub type CFDictionaryRef = *mut c_void;
    pub type CFSetRef = *mut c_void;
    pub type CFArrayRef = *mut c_void;
    pub type CFTypeRef = *mut c_void;
    pub type CFAllocatorRef = *mut c_void;
    pub type CFIndex = isize;
    pub type CFNumberRef = *mut c_void;

    pub const K_CF_ALLOCATOR_DEFAULT: CFAllocatorRef = std::ptr::null_mut();

    // IOHIDManager options
    pub const K_IO_HID_OPTIONS_TYPE_NONE: u32 = 0;
    pub const K_IO_HID_OPTIONS_TYPE_SEIZE_DEVICE: u32 = 1;

    // HID usage pages
    pub const K_HID_PAGE_GENERIC_DESKTOP: u32 = 0x01;
    pub const K_HID_PAGE_KEYBOARD: u32 = 0x07;
    pub const K_HID_PAGE_BUTTON: u32 = 0x09;
    pub const K_HID_PAGE_CONSUMER: u32 = 0x0C;

    // Generic desktop usages
    pub const K_HID_USAGE_GD_POINTER: u32 = 0x01;
    pub const K_HID_USAGE_GD_MOUSE: u32 = 0x02;
    pub const K_HID_USAGE_GD_KEYBOARD: u32 = 0x06;
    pub const K_HID_USAGE_GD_KEYPAD: u32 = 0x07;
    pub const K_HID_USAGE_GD_X: u32 = 0x30;
    pub const K_HID_USAGE_GD_Y: u32 = 0x31;
    pub const K_HID_USAGE_GD_WHEEL: u32 = 0x38;

    // Element types
    pub const K_IO_HID_ELEMENT_TYPE_INPUT_MISC: u32 = 1;
    pub const K_IO_HID_ELEMENT_TYPE_INPUT_BUTTON: u32 = 2;
    pub const K_IO_HID_ELEMENT_TYPE_INPUT_AXIS: u32 = 3;

    // CFNumber types
    pub const K_CF_NUMBER_INT_TYPE: i32 = 9;
    pub const K_CF_NUMBER_SINT32_TYPE: i32 = 3;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        pub fn IOHIDManagerCreate(
            allocator: CFAllocatorRef,
            options: u32,
        ) -> IOHIDManagerRef;

        pub fn IOHIDManagerSetDeviceMatchingMultiple(
            manager: IOHIDManagerRef,
            matching: CFArrayRef,
        );

        pub fn IOHIDManagerScheduleWithRunLoop(
            manager: IOHIDManagerRef,
            run_loop: CFRunLoopRef,
            run_loop_mode: CFStringRef,
        );

        pub fn IOHIDManagerUnscheduleFromRunLoop(
            manager: IOHIDManagerRef,
            run_loop: CFRunLoopRef,
            run_loop_mode: CFStringRef,
        );

        pub fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: u32) -> i32;

        pub fn IOHIDManagerClose(manager: IOHIDManagerRef, options: u32) -> i32;

        pub fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;

        pub fn IOHIDManagerRegisterInputValueCallback(
            manager: IOHIDManagerRef,
            callback: Option<
                extern "C" fn(context: *mut c_void, result: i32, sender: *mut c_void, value: IOHIDValueRef),
            >,
            context: *mut c_void,
        );

        pub fn IOHIDManagerRegisterDeviceMatchingCallback(
            manager: IOHIDManagerRef,
            callback: Option<
                extern "C" fn(context: *mut c_void, result: i32, sender: *mut c_void, device: IOHIDDeviceRef),
            >,
            context: *mut c_void,
        );

        pub fn IOHIDManagerRegisterDeviceRemovalCallback(
            manager: IOHIDManagerRef,
            callback: Option<
                extern "C" fn(context: *mut c_void, result: i32, sender: *mut c_void, device: IOHIDDeviceRef),
            >,
            context: *mut c_void,
        );

        pub fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: u32) -> i32;

        pub fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: u32) -> i32;

        pub fn IOHIDDeviceGetProperty(
            device: IOHIDDeviceRef,
            key: CFStringRef,
        ) -> CFTypeRef;

        pub fn IOHIDDeviceRegisterInputValueCallback(
            device: IOHIDDeviceRef,
            callback: Option<
                extern "C" fn(context: *mut c_void, result: i32, sender: *mut c_void, value: IOHIDValueRef),
            >,
            context: *mut c_void,
        );

        pub fn IOHIDDeviceScheduleWithRunLoop(
            device: IOHIDDeviceRef,
            run_loop: CFRunLoopRef,
            run_loop_mode: CFStringRef,
        );

        pub fn IOHIDDeviceUnscheduleFromRunLoop(
            device: IOHIDDeviceRef,
            run_loop: CFRunLoopRef,
            run_loop_mode: CFStringRef,
        );

        pub fn IOHIDValueGetElement(value: IOHIDValueRef) -> IOHIDElementRef;

        pub fn IOHIDValueGetIntegerValue(value: IOHIDValueRef) -> isize;

        pub fn IOHIDElementGetUsagePage(element: IOHIDElementRef) -> u32;

        pub fn IOHIDElementGetUsage(element: IOHIDElementRef) -> u32;

        pub fn IOHIDElementGetType(element: IOHIDElementRef) -> u32;

        pub fn IOHIDElementGetLogicalMin(element: IOHIDElementRef) -> isize;

        pub fn IOHIDElementGetLogicalMax(element: IOHIDElementRef) -> isize;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub static kCFRunLoopDefaultMode: CFStringRef;

        pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;

        pub fn CFRunLoopRunInMode(
            mode: CFStringRef,
            seconds: f64,
            return_after_source_handled: bool,
        ) -> i32;

        pub fn CFRunLoopStop(run_loop: CFRunLoopRef);

        pub fn CFRelease(cf: CFTypeRef);

        pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;

        pub fn CFSetGetCount(set: CFSetRef) -> CFIndex;

        pub fn CFSetGetValues(set: CFSetRef, values: *mut *const c_void);

        pub fn CFArrayCreate(
            allocator: CFAllocatorRef,
            values: *const CFTypeRef,
            num_values: CFIndex,
            callbacks: *const c_void,
        ) -> CFArrayRef;

        pub fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;

        pub fn CFArrayGetValueAtIndex(array: CFArrayRef, idx: CFIndex) -> *const c_void;

        pub fn CFDictionaryCreate(
            allocator: CFAllocatorRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            num_values: CFIndex,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CFDictionaryRef;

        pub fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            c_str: *const i8,
            encoding: u32,
        ) -> CFStringRef;

        pub fn CFStringGetCString(
            string: CFStringRef,
            buffer: *mut i8,
            buffer_size: CFIndex,
            encoding: u32,
        ) -> bool;

        pub fn CFStringGetLength(string: CFStringRef) -> CFIndex;

        pub fn CFNumberCreate(
            allocator: CFAllocatorRef,
            the_type: i32,
            value_ptr: *const c_void,
        ) -> CFNumberRef;

        pub fn CFNumberGetValue(
            number: CFNumberRef,
            the_type: i32,
            value_ptr: *mut c_void,
        ) -> bool;

        pub fn CFGetTypeID(cf: CFTypeRef) -> usize;

        pub fn CFStringGetTypeID() -> usize;

        pub fn CFNumberGetTypeID() -> usize;
    }

    // kCFStringEncodingUTF8
    pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

    // IOKit property keys
    pub const K_IO_HID_PRODUCT_KEY: &[u8] = b"Product\0";
    pub const K_IO_HID_VENDOR_ID_KEY: &[u8] = b"VendorID\0";
    pub const K_IO_HID_PRODUCT_ID_KEY: &[u8] = b"ProductID\0";
    pub const K_IO_HID_LOCATION_ID_KEY: &[u8] = b"LocationID\0";
    pub const K_IO_HID_PRIMARY_USAGE_PAGE_KEY: &[u8] = b"PrimaryUsagePage\0";
    pub const K_IO_HID_PRIMARY_USAGE_KEY: &[u8] = b"PrimaryUsage\0";
    pub const K_IO_HID_DEVICE_USAGE_PAGE_KEY: &[u8] = b"DeviceUsagePage\0";
    pub const K_IO_HID_DEVICE_USAGE_KEY: &[u8] = b"DeviceUsage\0";
}

/// Check if Input Monitoring permission is available
fn check_input_monitoring_permission() -> bool {
    // We need to check if we can access HID devices
    // The most reliable way is to try to open an HID manager and see if we get devices
    unsafe {
        let manager = hid::IOHIDManagerCreate(hid::K_CF_ALLOCATOR_DEFAULT, hid::K_IO_HID_OPTIONS_TYPE_NONE);
        if manager.is_null() {
            return false;
        }

        // Set matching to keyboard devices
        let matching = create_hid_device_matching_array();
        hid::IOHIDManagerSetDeviceMatchingMultiple(manager, matching);

        let result = hid::IOHIDManagerOpen(manager, hid::K_IO_HID_OPTIONS_TYPE_NONE);
        if result != 0 {
            hid::CFRelease(manager);
            if !matching.is_null() {
                hid::CFRelease(matching as hid::CFTypeRef);
            }
            return false;
        }

        // Try to get devices - this will fail without permission
        let devices = hid::IOHIDManagerCopyDevices(manager);
        let has_permission = !devices.is_null() && hid::CFSetGetCount(devices) > 0;

        if !devices.is_null() {
            hid::CFRelease(devices as hid::CFTypeRef);
        }
        hid::IOHIDManagerClose(manager, hid::K_IO_HID_OPTIONS_TYPE_NONE);
        hid::CFRelease(manager);
        if !matching.is_null() {
            hid::CFRelease(matching as hid::CFTypeRef);
        }

        has_permission
    }
}

/// Prompt user for Input Monitoring permission
fn prompt_for_input_monitoring_permission() {
    warn!(
        "Input Monitoring permission is required for keyboard/mouse capture.\n\
         Please grant permission in:\n\
         System Settings > Privacy & Security > Input Monitoring\n\
         Add this application to the list and restart."
    );
}

/// Create matching dictionary array for keyboards and mice
fn create_hid_device_matching_array() -> hid::CFArrayRef {
    unsafe {
        let mut dictionaries: Vec<hid::CFDictionaryRef> = Vec::new();

        // Keyboard matching
        if let Some(dict) = create_matching_dictionary(
            hid::K_HID_PAGE_GENERIC_DESKTOP,
            hid::K_HID_USAGE_GD_KEYBOARD,
        ) {
            dictionaries.push(dict);
        }

        // Keypad matching
        if let Some(dict) = create_matching_dictionary(
            hid::K_HID_PAGE_GENERIC_DESKTOP,
            hid::K_HID_USAGE_GD_KEYPAD,
        ) {
            dictionaries.push(dict);
        }

        // Mouse matching
        if let Some(dict) = create_matching_dictionary(
            hid::K_HID_PAGE_GENERIC_DESKTOP,
            hid::K_HID_USAGE_GD_MOUSE,
        ) {
            dictionaries.push(dict);
        }

        // Pointer matching (trackpads)
        if let Some(dict) = create_matching_dictionary(
            hid::K_HID_PAGE_GENERIC_DESKTOP,
            hid::K_HID_USAGE_GD_POINTER,
        ) {
            dictionaries.push(dict);
        }

        if dictionaries.is_empty() {
            return std::ptr::null_mut();
        }

        let array = hid::CFArrayCreate(
            hid::K_CF_ALLOCATOR_DEFAULT,
            dictionaries.as_ptr() as *const hid::CFTypeRef,
            dictionaries.len() as hid::CFIndex,
            std::ptr::null(),
        );

        // Release the dictionaries (array retains them)
        for dict in dictionaries {
            hid::CFRelease(dict as hid::CFTypeRef);
        }

        array
    }
}

/// Create a matching dictionary for a specific usage page and usage
fn create_matching_dictionary(usage_page: u32, usage: u32) -> Option<hid::CFDictionaryRef> {
    unsafe {
        let usage_page_key = hid::CFStringCreateWithCString(
            hid::K_CF_ALLOCATOR_DEFAULT,
            hid::K_IO_HID_DEVICE_USAGE_PAGE_KEY.as_ptr() as *const i8,
            hid::K_CF_STRING_ENCODING_UTF8,
        );
        let usage_key = hid::CFStringCreateWithCString(
            hid::K_CF_ALLOCATOR_DEFAULT,
            hid::K_IO_HID_DEVICE_USAGE_KEY.as_ptr() as *const i8,
            hid::K_CF_STRING_ENCODING_UTF8,
        );

        if usage_page_key.is_null() || usage_key.is_null() {
            if !usage_page_key.is_null() {
                hid::CFRelease(usage_page_key as hid::CFTypeRef);
            }
            if !usage_key.is_null() {
                hid::CFRelease(usage_key as hid::CFTypeRef);
            }
            return None;
        }

        let usage_page_num =
            hid::CFNumberCreate(hid::K_CF_ALLOCATOR_DEFAULT, hid::K_CF_NUMBER_SINT32_TYPE, &usage_page as *const u32 as *const c_void);
        let usage_num =
            hid::CFNumberCreate(hid::K_CF_ALLOCATOR_DEFAULT, hid::K_CF_NUMBER_SINT32_TYPE, &usage as *const u32 as *const c_void);

        if usage_page_num.is_null() || usage_num.is_null() {
            hid::CFRelease(usage_page_key as hid::CFTypeRef);
            hid::CFRelease(usage_key as hid::CFTypeRef);
            if !usage_page_num.is_null() {
                hid::CFRelease(usage_page_num as hid::CFTypeRef);
            }
            if !usage_num.is_null() {
                hid::CFRelease(usage_num as hid::CFTypeRef);
            }
            return None;
        }

        let keys = [usage_page_key as hid::CFTypeRef, usage_key as hid::CFTypeRef];
        let values = [usage_page_num as hid::CFTypeRef, usage_num as hid::CFTypeRef];

        let dict = hid::CFDictionaryCreate(
            hid::K_CF_ALLOCATOR_DEFAULT,
            keys.as_ptr(),
            values.as_ptr(),
            2,
            std::ptr::null(),
            std::ptr::null(),
        );

        hid::CFRelease(usage_page_key as hid::CFTypeRef);
        hid::CFRelease(usage_key as hid::CFTypeRef);
        hid::CFRelease(usage_page_num as hid::CFTypeRef);
        hid::CFRelease(usage_num as hid::CFTypeRef);

        if dict.is_null() {
            None
        } else {
            Some(dict)
        }
    }
}

/// Get device property as string
fn get_device_string_property(device: hid::IOHIDDeviceRef, key: &[u8]) -> Option<String> {
    unsafe {
        let key_str = hid::CFStringCreateWithCString(
            hid::K_CF_ALLOCATOR_DEFAULT,
            key.as_ptr() as *const i8,
            hid::K_CF_STRING_ENCODING_UTF8,
        );
        if key_str.is_null() {
            return None;
        }

        let value = hid::IOHIDDeviceGetProperty(device, key_str);
        hid::CFRelease(key_str as hid::CFTypeRef);

        if value.is_null() {
            return None;
        }

        // Check if it's a string
        if hid::CFGetTypeID(value) != hid::CFStringGetTypeID() {
            return None;
        }

        let len = hid::CFStringGetLength(value as hid::CFStringRef);
        let mut buffer = vec![0i8; (len as usize + 1) * 4]; // UTF-8 can be up to 4 bytes per char

        if hid::CFStringGetCString(
            value as hid::CFStringRef,
            buffer.as_mut_ptr(),
            buffer.len() as hid::CFIndex,
            hid::K_CF_STRING_ENCODING_UTF8,
        ) {
            let cstr = std::ffi::CStr::from_ptr(buffer.as_ptr());
            cstr.to_str().ok().map(|s| s.to_string())
        } else {
            None
        }
    }
}

/// Get device property as integer
fn get_device_int_property(device: hid::IOHIDDeviceRef, key: &[u8]) -> Option<i32> {
    unsafe {
        let key_str = hid::CFStringCreateWithCString(
            hid::K_CF_ALLOCATOR_DEFAULT,
            key.as_ptr() as *const i8,
            hid::K_CF_STRING_ENCODING_UTF8,
        );
        if key_str.is_null() {
            return None;
        }

        let value = hid::IOHIDDeviceGetProperty(device, key_str);
        hid::CFRelease(key_str as hid::CFTypeRef);

        if value.is_null() {
            return None;
        }

        // Check if it's a number
        if hid::CFGetTypeID(value) != hid::CFNumberGetTypeID() {
            return None;
        }

        let mut result: i32 = 0;
        if hid::CFNumberGetValue(
            value as hid::CFNumberRef,
            hid::K_CF_NUMBER_SINT32_TYPE,
            &mut result as *mut i32 as *mut c_void,
        ) {
            Some(result)
        } else {
            None
        }
    }
}

/// Get device type from usage page and usage
fn get_device_type_from_usage(usage_page: u32, usage: u32) -> DeviceType {
    if usage_page == hid::K_HID_PAGE_GENERIC_DESKTOP {
        match usage {
            hid::K_HID_USAGE_GD_KEYBOARD | hid::K_HID_USAGE_GD_KEYPAD => DeviceType::Keyboard,
            hid::K_HID_USAGE_GD_MOUSE | hid::K_HID_USAGE_GD_POINTER => DeviceType::Mouse,
            _ => DeviceType::Other,
        }
    } else {
        DeviceType::Other
    }
}

/// HID device information
#[derive(Clone)]
struct HIDDeviceInfo {
    device_ref: hid::IOHIDDeviceRef,
    info: PlatformDeviceInfo,
}

// HIDDeviceInfo needs to be Send because we store it in Arc<Mutex<>>
// The IOHIDDeviceRef is a pointer to an IOKit object that can be accessed from any thread
// as long as we synchronize access properly (which we do via Mutex)
unsafe impl Send for HIDDeviceInfo {}
unsafe impl Sync for HIDDeviceInfo {}

/// Shared state for HID input callbacks
struct HIDInputState {
    events: Vec<PlatformInputEvent>,
    running: bool,
}

/// macOS input backend combining IOKit HID Manager and gilrs
pub struct MacOSInputBackend {
    gilrs: Arc<Mutex<Gilrs>>,
    hid_manager: hid::IOHIDManagerRef,
    hid_devices: Arc<RwLock<HashMap<String, HIDDeviceInfo>>>,
    has_hid_permission: bool,
}

// MacOSInputBackend needs explicit Send/Sync implementations
// IOHIDManagerRef can be used from any thread as long as properly synchronized
unsafe impl Send for MacOSInputBackend {}
unsafe impl Sync for MacOSInputBackend {}

impl MacOSInputBackend {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().expect("Failed to initialize gilrs");

        // Check for Input Monitoring permission
        let has_hid_permission = check_input_monitoring_permission();
        if !has_hid_permission {
            prompt_for_input_monitoring_permission();
        } else {
            info!("Input Monitoring permission granted - keyboard/mouse capture available");
        }

        // Create HID manager
        let hid_manager = unsafe {
            hid::IOHIDManagerCreate(hid::K_CF_ALLOCATOR_DEFAULT, hid::K_IO_HID_OPTIONS_TYPE_NONE)
        };

        let backend = Self {
            gilrs: Arc::new(Mutex::new(gilrs)),
            hid_manager,
            hid_devices: Arc::new(RwLock::new(HashMap::new())),
            has_hid_permission,
        };

        // Initialize HID manager if we have permission
        if has_hid_permission && !hid_manager.is_null() {
            backend.initialize_hid_manager();
        }

        backend
    }

    fn initialize_hid_manager(&self) {
        unsafe {
            let matching = create_hid_device_matching_array();
            hid::IOHIDManagerSetDeviceMatchingMultiple(self.hid_manager, matching);

            // Open the manager
            let result = hid::IOHIDManagerOpen(self.hid_manager, hid::K_IO_HID_OPTIONS_TYPE_NONE);
            if result != 0 {
                error!("Failed to open HID manager: {}", result);
                return;
            }

            // Enumerate existing devices
            self.enumerate_hid_devices();

            if !matching.is_null() {
                hid::CFRelease(matching as hid::CFTypeRef);
            }
        }
    }

    fn enumerate_hid_devices(&self) {
        unsafe {
            let device_set = hid::IOHIDManagerCopyDevices(self.hid_manager);
            if device_set.is_null() {
                debug!("No HID devices found");
                return;
            }

            let count = hid::CFSetGetCount(device_set);
            if count == 0 {
                hid::CFRelease(device_set as hid::CFTypeRef);
                return;
            }

            let mut device_refs: Vec<*const c_void> = vec![std::ptr::null(); count as usize];
            hid::CFSetGetValues(device_set, device_refs.as_mut_ptr());

            let mut devices = self.hid_devices.write().unwrap();

            for device_ref in device_refs {
                if device_ref.is_null() {
                    continue;
                }

                let device = device_ref as hid::IOHIDDeviceRef;
                if let Some(info) = self.create_device_info(device) {
                    debug!("Found HID device: {} ({})", info.info.name, info.info.id);
                    devices.insert(info.info.id.clone(), info);
                }
            }

            hid::CFRelease(device_set as hid::CFTypeRef);
        }
    }

    fn create_device_info(&self, device: hid::IOHIDDeviceRef) -> Option<HIDDeviceInfo> {
        let name = get_device_string_property(device, hid::K_IO_HID_PRODUCT_KEY)
            .unwrap_or_else(|| "Unknown HID Device".to_string());

        let vendor_id = get_device_int_property(device, hid::K_IO_HID_VENDOR_ID_KEY)
            .unwrap_or(0) as u16;

        let product_id = get_device_int_property(device, hid::K_IO_HID_PRODUCT_ID_KEY)
            .unwrap_or(0) as u16;

        let location_id = get_device_int_property(device, hid::K_IO_HID_LOCATION_ID_KEY)
            .unwrap_or(0);

        let usage_page = get_device_int_property(device, hid::K_IO_HID_PRIMARY_USAGE_PAGE_KEY)
            .unwrap_or(0) as u32;

        let usage = get_device_int_property(device, hid::K_IO_HID_PRIMARY_USAGE_KEY)
            .unwrap_or(0) as u32;

        let device_type = get_device_type_from_usage(usage_page, usage);

        // Create a unique ID based on vendor, product, and location
        let id = format!("hid:{:04x}:{:04x}:{:08x}", vendor_id, product_id, location_id);

        Some(HIDDeviceInfo {
            device_ref: device,
            info: PlatformDeviceInfo {
                id,
                name,
                vendor_id,
                product_id,
                device_type,
                path: None,
                supports_grab: false, // macOS doesn't support device grabbing in the traditional sense
            },
        })
    }
}

impl Default for MacOSInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MacOSInputBackend {
    fn drop(&mut self) {
        if !self.hid_manager.is_null() {
            unsafe {
                hid::IOHIDManagerClose(self.hid_manager, hid::K_IO_HID_OPTIONS_TYPE_NONE);
                hid::CFRelease(self.hid_manager);
            }
        }
    }
}

#[async_trait]
impl InputBackend for MacOSInputBackend {
    async fn list_devices(&self) -> Result<Vec<PlatformDeviceInfo>> {
        let mut devices = Vec::new();

        // Add HID devices (keyboards and mice)
        if self.has_hid_permission {
            // Re-enumerate to get fresh list
            self.enumerate_hid_devices();

            let hid_devices = self.hid_devices.read().unwrap();
            for (_, device_info) in hid_devices.iter() {
                devices.push(device_info.info.clone());
            }
        }

        // Add gamepad devices from gilrs
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

        Ok(devices)
    }

    async fn open_device(&self, device_id: &str) -> Result<Box<dyn PlatformInputDevice>> {
        if device_id.starts_with("gamepad:") {
            let id_str = device_id.strip_prefix("gamepad:").unwrap_or("0");
            let id: usize = id_str.parse().map_err(|_| {
                RemapperError::DeviceNotFound(format!("Invalid gamepad ID: {}", device_id))
            })?;

            let device = MacOSGamepadDevice::open(self.gilrs.clone(), id)?;
            Ok(Box::new(device))
        } else if device_id.starts_with("hid:") {
            if !self.has_hid_permission {
                return Err(RemapperError::PermissionDenied(
                    "Input Monitoring permission required for keyboard/mouse capture. \
                     Please grant permission in System Settings > Privacy & Security > Input Monitoring"
                        .to_string(),
                ));
            }

            let hid_devices = self.hid_devices.read().unwrap();
            let device_info = hid_devices.get(device_id).ok_or_else(|| {
                RemapperError::DeviceNotFound(format!("HID device not found: {}", device_id))
            })?;

            let device = MacOSHIDDevice::open(device_info.clone())?;
            Ok(Box::new(device))
        } else {
            Err(RemapperError::DeviceNotFound(format!(
                "Unknown device type: {}. Expected 'gamepad:' or 'hid:' prefix",
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

/// macOS HID input device for keyboard and mouse
pub struct MacOSHIDDevice {
    device_info: HIDDeviceInfo,
    events: Arc<Mutex<Vec<PlatformInputEvent>>>,
    running: Arc<AtomicBool>,
    grabbed: bool,
}

// HID callbacks need raw pointers, so we implement Send/Sync manually
unsafe impl Send for MacOSHIDDevice {}
unsafe impl Sync for MacOSHIDDevice {}

impl MacOSHIDDevice {
    fn open(device_info: HIDDeviceInfo) -> Result<Self> {
        debug!("Opening HID device: {} ({})", device_info.info.name, device_info.info.id);

        // Open the device
        let result = unsafe {
            hid::IOHIDDeviceOpen(device_info.device_ref, hid::K_IO_HID_OPTIONS_TYPE_NONE)
        };

        if result != 0 {
            return Err(RemapperError::DeviceNotFound(format!(
                "Failed to open HID device: {} (error {})",
                device_info.info.name, result
            )));
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(true));

        let device = Self {
            device_info,
            events,
            running,
            grabbed: false,
        };

        // Register input callback
        device.register_input_callback();

        Ok(device)
    }

    fn register_input_callback(&self) {
        // Store callback context
        let events = self.events.clone();
        let running = self.running.clone();
        let device_type = self.device_info.info.device_type;

        // Create a context struct to pass to the callback
        let context = Box::new(HIDCallbackContext {
            events,
            running,
            device_type,
        });

        let context_ptr = Box::into_raw(context);

        unsafe {
            // Register the callback
            hid::IOHIDDeviceRegisterInputValueCallback(
                self.device_info.device_ref,
                Some(hid_input_value_callback),
                context_ptr as *mut c_void,
            );

            // Schedule with run loop
            hid::IOHIDDeviceScheduleWithRunLoop(
                self.device_info.device_ref,
                hid::CFRunLoopGetCurrent(),
                hid::kCFRunLoopDefaultMode,
            );
        }
    }
}

struct HIDCallbackContext {
    events: Arc<Mutex<Vec<PlatformInputEvent>>>,
    running: Arc<AtomicBool>,
    device_type: DeviceType,
}

extern "C" fn hid_input_value_callback(
    context: *mut c_void,
    _result: i32,
    _sender: *mut c_void,
    value: hid::IOHIDValueRef,
) {
    if context.is_null() || value.is_null() {
        return;
    }

    let ctx = unsafe { &*(context as *const HIDCallbackContext) };

    if !ctx.running.load(Ordering::Relaxed) {
        return;
    }

    unsafe {
        let element = hid::IOHIDValueGetElement(value);
        if element.is_null() {
            return;
        }

        let usage_page = hid::IOHIDElementGetUsagePage(element);
        let usage = hid::IOHIDElementGetUsage(element);
        let int_value = hid::IOHIDValueGetIntegerValue(value) as i32;

        // Convert HID event to platform event
        if let Some(event) = hid_to_platform_event(ctx.device_type, usage_page, usage, int_value) {
            if let Ok(mut events) = ctx.events.lock() {
                events.push(event);
            }
        }
    }
}

/// Convert HID event to platform event
fn hid_to_platform_event(
    device_type: DeviceType,
    usage_page: u32,
    usage: u32,
    value: i32,
) -> Option<PlatformInputEvent> {
    match device_type {
        DeviceType::Keyboard => {
            // Keyboard events
            if usage_page == hid::K_HID_PAGE_KEYBOARD && (4..=231).contains(&usage) {
                // Convert HID keyboard usage to evdev keycode
                let evdev_code = hid_keyboard_to_evdev(usage);
                // value: 1 = pressed, 0 = released
                let event_value = if value != 0 { 1 } else { 0 };
                Some(PlatformInputEvent::new(1, evdev_code, event_value))
            } else {
                None
            }
        }
        DeviceType::Mouse => {
            if usage_page == hid::K_HID_PAGE_GENERIC_DESKTOP {
                match usage {
                    hid::K_HID_USAGE_GD_X => {
                        // Mouse X movement (REL_X = 0)
                        Some(PlatformInputEvent::new(2, 0, value))
                    }
                    hid::K_HID_USAGE_GD_Y => {
                        // Mouse Y movement (REL_Y = 1)
                        Some(PlatformInputEvent::new(2, 1, value))
                    }
                    hid::K_HID_USAGE_GD_WHEEL => {
                        // Scroll wheel (REL_WHEEL = 8)
                        Some(PlatformInputEvent::new(2, 8, value))
                    }
                    _ => None,
                }
            } else if usage_page == hid::K_HID_PAGE_BUTTON {
                // Mouse buttons
                let button_code = match usage {
                    1 => 272, // BTN_LEFT
                    2 => 273, // BTN_RIGHT
                    3 => 274, // BTN_MIDDLE
                    4 => 275, // BTN_SIDE
                    5 => 276, // BTN_EXTRA
                    _ => return None,
                };
                let event_value = if value != 0 { 1 } else { 0 };
                Some(PlatformInputEvent::new(1, button_code, event_value))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert HID keyboard usage code to evdev keycode
fn hid_keyboard_to_evdev(hid_usage: u32) -> u16 {
    // HID usage codes to evdev keycodes mapping
    // HID usages 4-29 = A-Z
    // HID usages 30-39 = 1-0
    // etc.
    match hid_usage {
        // Letters A-Z (HID 4-29 -> evdev KEY_A-KEY_Z)
        4 => 30,   // A
        5 => 48,   // B
        6 => 46,   // C
        7 => 32,   // D
        8 => 18,   // E
        9 => 33,   // F
        10 => 34,  // G
        11 => 35,  // H
        12 => 23,  // I
        13 => 36,  // J
        14 => 37,  // K
        15 => 38,  // L
        16 => 50,  // M
        17 => 49,  // N
        18 => 24,  // O
        19 => 25,  // P
        20 => 16,  // Q
        21 => 19,  // R
        22 => 31,  // S
        23 => 20,  // T
        24 => 22,  // U
        25 => 47,  // V
        26 => 17,  // W
        27 => 45,  // X
        28 => 21,  // Y
        29 => 44,  // Z

        // Numbers 1-0 (HID 30-39)
        30 => 2,   // 1
        31 => 3,   // 2
        32 => 4,   // 3
        33 => 5,   // 4
        34 => 6,   // 5
        35 => 7,   // 6
        36 => 8,   // 7
        37 => 9,   // 8
        38 => 10,  // 9
        39 => 11,  // 0

        // Special keys
        40 => 28,  // Enter
        41 => 1,   // Escape
        42 => 14,  // Backspace
        43 => 15,  // Tab
        44 => 57,  // Space
        45 => 12,  // Minus
        46 => 13,  // Equal
        47 => 26,  // LeftBracket
        48 => 27,  // RightBracket
        49 => 43,  // Backslash
        51 => 39,  // Semicolon
        52 => 40,  // Quote
        53 => 41,  // Grave
        54 => 51,  // Comma
        55 => 52,  // Period
        56 => 53,  // Slash
        57 => 58,  // CapsLock

        // Function keys
        58 => 59,  // F1
        59 => 60,  // F2
        60 => 61,  // F3
        61 => 62,  // F4
        62 => 63,  // F5
        63 => 64,  // F6
        64 => 65,  // F7
        65 => 66,  // F8
        66 => 67,  // F9
        67 => 68,  // F10
        68 => 87,  // F11
        69 => 88,  // F12

        // Control keys
        70 => 99,  // PrintScreen
        71 => 70,  // ScrollLock
        72 => 119, // Pause
        73 => 110, // Insert
        74 => 102, // Home
        75 => 104, // PageUp
        76 => 111, // Delete
        77 => 107, // End
        78 => 109, // PageDown
        79 => 106, // Right
        80 => 105, // Left
        81 => 108, // Down
        82 => 103, // Up

        // Numpad
        83 => 69,  // NumLock
        84 => 98,  // Keypad /
        85 => 55,  // Keypad *
        86 => 74,  // Keypad -
        87 => 78,  // Keypad +
        88 => 96,  // Keypad Enter
        89 => 79,  // Keypad 1
        90 => 80,  // Keypad 2
        91 => 81,  // Keypad 3
        92 => 75,  // Keypad 4
        93 => 76,  // Keypad 5
        94 => 77,  // Keypad 6
        95 => 71,  // Keypad 7
        96 => 72,  // Keypad 8
        97 => 73,  // Keypad 9
        98 => 82,  // Keypad 0
        99 => 83,  // Keypad .

        // Modifiers
        224 => 29,  // Left Control
        225 => 42,  // Left Shift
        226 => 56,  // Left Alt
        227 => 125, // Left Meta (Command)
        228 => 97,  // Right Control
        229 => 54,  // Right Shift
        230 => 100, // Right Alt
        231 => 126, // Right Meta (Command)

        _ => hid_usage as u16, // Fallback
    }
}

#[async_trait]
impl PlatformInputDevice for MacOSHIDDevice {
    async fn read_event(&mut self) -> Result<Option<PlatformInputEvent>> {
        // Run the run loop briefly to process callbacks
        unsafe {
            hid::CFRunLoopRunInMode(hid::kCFRunLoopDefaultMode, 0.001, false);
        }

        // Check for events - scope the lock to avoid holding across await
        let event = {
            let mut events = self.events.lock().map_err(|_| {
                RemapperError::EventReadError("Failed to lock events".to_string())
            })?;
            events.pop()
        };

        if let Some(event) = event {
            trace!("Read HID event: {:?}", event);
            Ok(Some(event))
        } else {
            // Small sleep to prevent busy-waiting
            tokio::time::sleep(Duration::from_millis(1)).await;
            Ok(None)
        }
    }

    async fn grab(&mut self) -> Result<()> {
        // macOS doesn't support device grabbing in the traditional sense
        // We can try to open with seize option but it may not work for all devices
        if !self.grabbed {
            warn!(
                "Device grabbing is not fully supported on macOS. \
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
        &self.device_info.info
    }

    fn capabilities(&self) -> DeviceCapabilities {
        match self.device_info.info.device_type {
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

impl Drop for MacOSHIDDevice {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        unsafe {
            // Unschedule from run loop
            hid::IOHIDDeviceUnscheduleFromRunLoop(
                self.device_info.device_ref,
                hid::CFRunLoopGetCurrent(),
                hid::kCFRunLoopDefaultMode,
            );

            // Remove callback
            hid::IOHIDDeviceRegisterInputValueCallback(
                self.device_info.device_ref,
                None,
                std::ptr::null_mut(),
            );

            // Close device
            hid::IOHIDDeviceClose(self.device_info.device_ref, hid::K_IO_HID_OPTIONS_TYPE_NONE);
        }
    }
}

/// macOS gamepad input device using gilrs (unchanged from original)
pub struct MacOSGamepadDevice {
    gilrs: Arc<Mutex<Gilrs>>,
    gamepad_id: gilrs::GamepadId,
    info: PlatformDeviceInfo,
    grabbed: bool,
}

impl MacOSGamepadDevice {
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
impl PlatformInputDevice for MacOSGamepadDevice {
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
        // macOS doesn't support device grabbing
        if !self.grabbed {
            warn!(
                "Device grabbing is not supported on macOS. \
                Other applications will still receive input from this device."
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
            304, 305, 306, 307, // BTN_SOUTH, EAST, C, NORTH
            308, 309,           // BTN_WEST, Z
            310, 311,           // BTN_TL, BTN_TR
            312, 313,           // BTN_TL2, BTN_TR2
            314, 315,           // BTN_SELECT, BTN_START
            316, 317, 318,      // BTN_MODE, BTN_THUMBL, BTN_THUMBR
        ];

        let abs_axes = vec![
            AbsAxisInfo {
                code: 0,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 1,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 3,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 4,
                value: 0,
                minimum: -32768,
                maximum: 32767,
                fuzz: 16,
                flat: 128,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 2,
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 5,
                value: 0,
                minimum: 0,
                maximum: 255,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 16,
                value: 0,
                minimum: -1,
                maximum: 1,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            AbsAxisInfo {
                code: 17,
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

/// Convert gilrs event to platform event
fn gilrs_event_to_platform(event: &Event) -> Option<PlatformInputEvent> {
    match event.event {
        EventType::ButtonPressed(button, _) => {
            let code = button_to_code(button)?;
            Some(PlatformInputEvent::new(1, code, 1))
        }
        EventType::ButtonReleased(button, _) => {
            let code = button_to_code(button)?;
            Some(PlatformInputEvent::new(1, code, 0))
        }
        EventType::ButtonChanged(button, value, _) => {
            let code = button_to_code(button)?;
            let int_value = (value * 255.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value))
        }
        EventType::AxisChanged(axis, value, _) => {
            let code = axis_to_code(axis)?;
            let int_value = (value * 32767.0) as i32;
            Some(PlatformInputEvent::new(3, code, int_value))
        }
        EventType::Connected | EventType::Disconnected | EventType::Dropped => None,
        _ => None,
    }
}

fn button_to_code(button: Button) -> Option<u16> {
    match button {
        Button::South => Some(304),
        Button::East => Some(305),
        Button::North => Some(307),
        Button::West => Some(308),
        Button::LeftTrigger => Some(310),
        Button::RightTrigger => Some(311),
        Button::LeftTrigger2 => Some(312),
        Button::RightTrigger2 => Some(313),
        Button::Select => Some(314),
        Button::Start => Some(315),
        Button::Mode => Some(316),
        Button::LeftThumb => Some(317),
        Button::RightThumb => Some(318),
        Button::C => Some(306),
        Button::Z => Some(309),
        Button::DPadUp | Button::DPadDown | Button::DPadLeft | Button::DPadRight => Some(0),
        Button::Unknown => None,
    }
}

fn axis_to_code(axis: Axis) -> Option<u16> {
    match axis {
        Axis::LeftStickX => Some(0),
        Axis::LeftStickY => Some(1),
        Axis::RightStickX => Some(3),
        Axis::RightStickY => Some(4),
        Axis::LeftZ => Some(2),
        Axis::RightZ => Some(5),
        Axis::DPadX => Some(16),
        Axis::DPadY => Some(17),
        Axis::Unknown => None,
    }
}
