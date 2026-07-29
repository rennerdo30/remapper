# Remapper

[![CI](https://github.com/rennerdo30/remapper/actions/workflows/main.yml/badge.svg)](https://github.com/rennerdo30/remapper/actions/workflows/main.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

Cross-platform input remapping tool written in Rust.

Remapper lets you map input events from physical devices (keyboard, mouse, gamepad) to virtual output devices, with support for simple remaps, macros, combos, and tap/hold behavior.

## Features

- Simple 1:1 remapping
- Macro sequences
- Tap/hold conditional mappings
- Combo/chord mappings
- CLI workflow for creating and running profiles
- GUI built with `iced`
- Background daemon, plus systemd user service management on Linux
- Hot-plug device monitoring
- JSON configuration with migration support

## Platform support

| Platform | Input | Virtual output |
|----------|-------|----------------|
| Linux | `evdev` (`/dev/input/*`) | `uinput` (`/dev/uinput`) - keyboard, mouse, gamepad |
| Windows | Raw Input API | `SendInput` for keyboard/mouse, gamepads via the [ViGEmBus](https://github.com/nefarius/ViGEmBus) driver |
| macOS | IOKit HID Manager | Core Graphics events for keyboard/mouse - virtual gamepad output is not supported |

Gamepads are additionally read through `gilrs` on all platforms.

## Installation

### Build from source

```bash
git clone https://github.com/rennerdo30/remapper.git
cd remapper
cargo build --release
```

The binary will be at `target/release/remapper`.

### Linux runtime requirements

- `libudev`
- access to `/dev/input/*`
- access to `/dev/uinput`

Optional udev rules are provided in `udev/99-remapper.rules`. A `PKGBUILD` for Arch-based
distributions and unit files under `systemd/` are included as well.

### Windows

Virtual gamepad output requires the ViGEmBus driver. Without it, keyboard and mouse output still
work and gamepad mappings are reported as unsupported.

### macOS

Capturing keyboard and mouse events requires the *Input Monitoring* permission
(System Settings > Privacy & Security > Input Monitoring).

## Quick Start

```bash
# List available devices
remapper list devices

# Create a profile interactively
remapper create

# Run all enabled profiles
remapper run

# Launch GUI
remapper gui
```

## Commands

| Command | Purpose |
|---------|---------|
| `remapper create [name] [--device <path-or-name>]` | Create a profile interactively |
| `remapper edit <name>` | Edit an existing profile |
| `remapper delete <name>` | Delete a profile |
| `remapper list devices` / `list profiles` | List input devices or configured profiles |
| `remapper run [profiles...] [--daemon]` | Run enabled profiles, optionally in the background |
| `remapper debug <device>` | Print raw events from a device |
| `remapper gui` | Launch the graphical interface |
| `remapper service <action>` | Manage the systemd user service (Linux only) |

`remapper service` supports `install`, `uninstall`, `start`, `stop`, `restart`, `status`,
`logs [--lines N] [--follow]`, `enable` and `disable`.

Global flag: `-v/--verbose`. See `remapper --help` for the authoritative list.

## Configuration

Profiles and mappings live in a single `config.json` inside the platform config directory
(resolved with the `directories` crate):

- Linux: `~/.config/remapper/config.json`
- macOS: `~/Library/Application Support/remapper/config.json`
- Windows: `%APPDATA%\remapper\config.json`

Older v1 configuration files are migrated automatically on load; the original file is kept
alongside as `config.v1.bak.json`, and timestamped backups are written before saving.

Example fixtures are available in `tests/fixtures/`.

## Development

```bash
# Typecheck and compile
cargo check

# Run tests
cargo test
```

GitHub Actions runs CI on pushes and pull requests.

> The Python files in the repository root (`remapper.py`, `gui.py`, `*_ui.py`, `requirements.txt`,
> `setup.py`, ...) are the previous 1.x implementation. The current tool is the Rust crate in
> `src/`; the Python version is no longer maintained.

## Project Docs

- Contributing guide: [CONTRIBUTING.md](CONTRIBUTING.md)
- Code of conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- Security policy: [SECURITY.md](SECURITY.md)
- Support: [SUPPORT.md](SUPPORT.md)
- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Releasing: [docs/RELEASING.md](docs/RELEASING.md)

## License

Licensed under GPL-3.0. See [LICENSE](LICENSE).
