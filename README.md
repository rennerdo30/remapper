# Remapper

A Linux tool for remapping evdev (event device) inputs to different outputs via a virtual input device. Supports both GUI and CLI modes.

## Features

- **Simple Remapping**: 1:1 key/button remapping (e.g., swap A and B buttons)
- **Macros**: Trigger sequences of key events with a single button
- **Tap/Hold**: Different actions for tap vs. hold (e.g., tap for Escape, hold for Meta)
- **Combo/Chords**: Trigger actions when multiple keys are pressed together
- **GUI Interface**: Modern graphical interface built with iced
- **CLI Tools**: Interactive wizard, debug mode, and configuration management
- **Systemd Integration**: Run as a user or system service
- **Hotplug Support**: Automatic device detection

## Installation

### From Source (Rust)

```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/user/remapper.git
cd remapper
cargo build --release

# Install
sudo cp target/release/remapper /usr/local/bin/
```

### Arch Linux (AUR)

```bash
yay -S remapper
# or
paru -S remapper
```

## Quick Start

```bash
# List available input devices
remapper list devices

# Create a new profile interactively
remapper create

# Run all enabled profiles
remapper run

# Launch the GUI
remapper gui
```

## Usage

### CLI Commands

```
remapper - Linux evdev input remapping tool

COMMANDS:
    create           Create a new profile (interactive wizard)
    edit <NAME>      Edit an existing profile
    delete <NAME>    Delete a profile
    list devices     List available input devices
    list profiles    List configured profiles
    run [PROFILES]   Run profiles (all enabled if none specified)
    run --daemon     Run as background daemon
    debug <DEVICE>   Show raw events from a device
    gui              Launch graphical interface
    service install  Install systemd user service
    service start    Start the service
    service stop     Stop the service
    service status   Show service status

OPTIONS:
    -v, --verbose    Enable verbose output
    -h, --help       Print help
    -V, --version    Print version
```

### Debug Mode

Debug mode shows raw events from a device - useful for finding key codes:

```bash
remapper debug "Xbox Controller"
# or
remapper debug /dev/input/event5
```

### Running as a Service

```bash
# Install the systemd user service
remapper service install

# Enable to start on login
remapper service enable

# Start now
remapper service start

# Check status
remapper service status
```

## Configuration

Configuration is stored in `~/.config/remapper/config.json`.

### Example Configuration

```json
{
  "version": 2,
  "settings": {
    "log_level": "info",
    "auto_start": true,
    "hotplug_enabled": true
  },
  "profiles": [
    {
      "name": "Xbox Controller Remap",
      "enabled": true,
      "input_device": {
        "name": "Microsoft X-Box One pad"
      },
      "output_device": {
        "name": "Remapped Xbox Controller"
      },
      "grab": false,
      "mappings": [
        {
          "type": "simple",
          "from": { "type": "EV_KEY", "code": "BTN_A" },
          "to": { "type": "EV_KEY", "code": "BTN_B" }
        },
        {
          "type": "conditional",
          "trigger": { "type": "EV_KEY", "code": "BTN_START" },
          "tap": { "type": "EV_KEY", "code": "KEY_ESC" },
          "hold": { "type": "EV_KEY", "code": "KEY_LEFTMETA" },
          "threshold_ms": 300
        }
      ]
    }
  ]
}
```

### Mapping Types

#### Simple (1:1)

```json
{
  "type": "simple",
  "from": { "type": "EV_KEY", "code": "BTN_A" },
  "to": { "type": "EV_KEY", "code": "BTN_B" }
}
```

#### Macro (Key Sequence)

```json
{
  "type": "macro",
  "trigger": { "type": "EV_KEY", "code": "BTN_SELECT" },
  "sequence": [
    { "code": "KEY_LEFTALT", "value": 1 },
    { "code": "KEY_TAB", "value": 1 },
    { "delay_ms": 50 },
    { "code": "KEY_TAB", "value": 0 },
    { "code": "KEY_LEFTALT", "value": 0 }
  ]
}
```

#### Conditional (Tap/Hold)

```json
{
  "type": "conditional",
  "trigger": { "type": "EV_KEY", "code": "BTN_START" },
  "tap": { "type": "EV_KEY", "code": "KEY_ESC" },
  "hold": { "type": "EV_KEY", "code": "KEY_LEFTMETA" },
  "threshold_ms": 300
}
```

#### Combo (Chord)

```json
{
  "type": "combo",
  "keys": [
    { "type": "EV_KEY", "code": "BTN_TL" },
    { "type": "EV_KEY", "code": "BTN_TR" }
  ],
  "output": { "type": "EV_KEY", "code": "KEY_ESC" },
  "order_sensitive": false
}
```

## Permissions

Remapper needs access to `/dev/input/` devices and `/dev/uinput`. You can either:

1. **Run as root** (not recommended for regular use)
2. **Add user to input group** (recommended):

```bash
sudo usermod -aG input $USER
# Log out and back in for changes to take effect
```

3. **Install udev rules** (included):

```bash
sudo cp udev/99-remapper.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## Building from Source

### Requirements

- Rust 1.70+ (with cargo)
- Linux with evdev support
- libudev (for device discovery)

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with verbose logging
RUST_LOG=debug cargo run -- list devices
```

## Project Structure

```
remapper/
├── Cargo.toml              # Rust package manifest
├── src/
│   ├── main.rs             # Entry point
│   ├── lib.rs              # Library exports
│   ├── core/               # Core engine and types
│   ├── devices/            # Input/output device handling
│   ├── config/             # Configuration management
│   ├── mappings/           # Mapping handlers
│   ├── cli/                # CLI commands
│   ├── gui/                # iced GUI
│   └── daemon/             # Daemon mode
├── systemd/                # Systemd service files
├── udev/                   # udev rules
└── tests/                  # Integration tests
```

## Migrating from Python Version

If you have an existing configuration from the Python version, Remapper will automatically migrate it to the new format on first run. A backup of your old configuration will be saved as `config.v1.bak.json`.

## License

GPL-3.0 - See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
