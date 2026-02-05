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
- JSON configuration with migration support

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

Optional udev rules are provided in `udev/99-remapper.rules`.

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

See all commands:

```bash
remapper --help
```

## Configuration

User configuration is stored in:

- `~/.config/remapper/config.json`

Example fixtures are available in `tests/fixtures/`.

## Development

```bash
# Typecheck and compile
cargo check

# Run tests
cargo test
```

GitHub Actions runs CI on pushes and pull requests.

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
