# Remapper Specification

## Overview

Remapper is a Linux application that enables remapping of evdev input events to different output events through a virtual input device. It intercepts events from physical input devices (keyboards, gamepads, mice) and translates them to user-defined outputs.

## Functional Requirements

### FR1: Input Device Management

- **FR1.1**: List all available evdev input devices from `/dev/input/`
- **FR1.2**: Display device information (name, vendor ID, product ID, version, bus type)
- **FR1.3**: Support device selection via GUI or CLI
- **FR1.4**: Support optional exclusive device grab (prevents other applications from receiving input)

### FR2: Output Device Management

- **FR2.1**: Create virtual output devices using uinput kernel module
- **FR2.2**: Configure virtual device properties (name, vendor ID, product ID, version, bus type)
- **FR2.3**: Support all evdev event types (EV_KEY, EV_ABS, EV_REL, EV_FF, EV_LED, etc.)

### FR3: Event Remapping

- **FR3.1**: Map input event codes to different output event codes
- **FR3.2**: Support same-type mapping (key-to-key, button-to-button, axis-to-axis)
- **FR3.3**: Support cross-type mapping where applicable
- **FR3.4**: Process events in real-time with minimal latency
- **FR3.5**: Run remapping in background threads

### FR4: Configuration Management

- **FR4.1**: Save remap configurations to persistent storage
- **FR4.2**: Load remap configurations on application start
- **FR4.3**: Support multiple independent remap profiles
- **FR4.4**: Store configuration in JSON format at `~/.config/remapper/config.json`
- **FR4.5**: Support CRUD operations for remap profiles

### FR5: User Interfaces

#### FR5.1: GUI Mode (PyQt5)
- Main window displaying configured remaps
- Add/edit/delete remap dialogs
- Device selection with detailed information
- Event code selection via dropdown menus
- Visual feedback for active remaps

#### FR5.2: CLI Mode
- Interactive setup wizard for creating remaps
- Device selection via numbered list
- Event capture for mapping configuration
- Text-based configuration display

#### FR5.3: Debug Mode
- Real-time event display from selected device
- Event type and code information
- Useful for identifying event codes to map

#### FR5.4: Run Mode
- Headless operation loading saved configurations
- Start all configured remaps as background services
- Suitable for system service deployment

### FR6: Device Presets

- **FR6.1**: Pre-configured device profiles (e.g., Xbox Gamepad)
- **FR6.2**: Preset vendor ID, product ID, and bus type values
- **FR6.3**: User-selectable presets in GUI

## Non-Functional Requirements

### NFR1: Performance
- Event processing latency < 1ms under normal conditions
- Support for high-frequency input devices (1000Hz+ polling rate)
- Efficient memory usage for continuous operation

### NFR2: Reliability
- Graceful handling of device disconnection
- Automatic cleanup of virtual devices on exit
- Configuration backup to prevent data loss

### NFR3: Platform Compatibility
- Linux-only (kernel 2.6+ with evdev/uinput support)
- Python 3.x compatible
- Distribution-agnostic installation

### NFR4: Security
- Requires appropriate permissions for input device access
- No network communication
- Local-only operation

## Technical Architecture

### Components

```
┌─────────────────────────────────────────────────────────────┐
│                      Application Layer                       │
├──────────────┬──────────────┬──────────────┬────────────────┤
│   GUI Mode   │   CLI Mode   │  Debug Mode  │   Run Mode     │
│   (gui.py)   │(remapper.py) │(remapper.py) │ (remapper.py)  │
└──────────────┴──────────────┴──────────────┴────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Core Components                         │
├──────────────────┬──────────────────┬───────────────────────┤
│  Config Manager  │  Remap Engine    │   Utility Functions   │
│   (config.py)    │   (remap.py)     │     (util.py)         │
└──────────────────┴──────────────────┴───────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Device Layer                            │
├────────────────────────────┬────────────────────────────────┤
│      Input Device          │       Output Device            │
│    (inputdevice.py)        │     (outputdevice.py)          │
└────────────────────────────┴────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      System Layer                            │
├────────────────────────────┬────────────────────────────────┤
│    evdev (InputDevice)     │      uinput (UInput)           │
│    /dev/input/*            │      /dev/uinput               │
└────────────────────────────┴────────────────────────────────┘
```

### Data Flow

1. **Input**: Physical device generates event
2. **Capture**: evdev reads event from /dev/input/eventX
3. **Translation**: Remap engine applies event_map transformation
4. **Output**: Translated event sent via uinput virtual device
5. **Delivery**: Kernel distributes event to applications

### Configuration Schema

```json
{
  "remaps": [
    {
      "name": "string",
      "input_device": {
        "path": "/dev/input/eventX",
        "name": "string",
        "vendor": "integer",
        "product": "integer"
      },
      "output_device": {
        "name": "string",
        "vendor": "integer",
        "product": "integer",
        "version": "integer",
        "bustype": "integer"
      },
      "event_map": {
        "input_code": "output_code"
      },
      "grab": "boolean"
    }
  ]
}
```

## Supported Event Types

| Event Type | Code | Description |
|------------|------|-------------|
| EV_SYN | 0x00 | Synchronization events |
| EV_KEY | 0x01 | Key/button press events |
| EV_REL | 0x02 | Relative axis events (mouse movement) |
| EV_ABS | 0x03 | Absolute axis events (joystick, touchscreen) |
| EV_MSC | 0x04 | Miscellaneous events |
| EV_SW | 0x05 | Switch events |
| EV_LED | 0x11 | LED control events |
| EV_SND | 0x12 | Sound events |
| EV_REP | 0x14 | Auto-repeat events |
| EV_FF | 0x15 | Force feedback events |

## Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| PyQt5 | >= 5.15 | GUI framework |
| PyQt5_sip | 12.8.0 | PyQt5 bindings |
| evdev | 1.3.0 | Linux input device library |
| Python | >= 3.6 | Runtime environment |

## System Requirements

- **OS**: Linux (kernel 2.6+)
- **Kernel Modules**: evdev, uinput
- **Permissions**: Read access to /dev/input/*, write access to /dev/uinput
- **User Group**: Typically `input` group membership or root privileges

## License

GPL-3.0 (GNU General Public License v3.0)
