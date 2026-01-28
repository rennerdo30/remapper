# CLAUDE.md

This file provides guidance for Claude Code when working with this repository.

## Project Overview

Remapper is a Linux tool for remapping evdev (event device) inputs to different outputs via a virtual input device. It supports both GUI (PyQt5) and CLI modes.

## Build & Install

```bash
# Install dependencies
pip install -r requirements.txt

# Install from source
python setup.py install

# Arch Linux package build
makepkg -si
```

## Running the Application

```bash
# GUI mode (default)
python remapper.py

# CLI mode - interactive setup wizard
python remapper.py --cli

# Debug mode - display raw device events
python remapper.py --debug

# Run mode - start configured remaps
python remapper.py --run

# Capture mode - capture and log events
python remapper.py --capture
```

## Project Structure

All source files are in the root directory:

- `remapper.py` - Main entry point, CLI argument parsing
- `gui.py` - PyQt5 GUI implementation
- `remap.py` - Core remapping engine with threading
- `config.py` - JSON configuration management (~/.config/remapper/config.json)
- `inputdevice.py` - evdev InputDevice wrapper
- `outputdevice.py` - Virtual uinput device wrapper
- `util.py` - Helper functions, presets, event code conversion
- `remapper_ui.py` / `add_remap_ui.py` - Generated PyQt5 UI code
- `remapper.ui` / `add_remap.ui` - Qt Designer UI definitions

## Key Technologies

- Python 3
- PyQt5 for GUI
- evdev library for Linux input device access
- uinput kernel module for virtual device creation

## Architecture Notes

- Input events are read from `/dev/input/*` devices using evdev
- Events are translated via an event_map dictionary
- Output events are sent through a virtual uinput device
- Configuration is stored as JSON in `~/.config/remapper/config.json`
- Remapping runs in background threads using asyncio

## Code Conventions

- Standard Python naming conventions (snake_case for functions/variables)
- PyQt5 signal/slot pattern for GUI events
- evdev event types/codes used throughout (EV_KEY, EV_ABS, etc.)

## Platform Requirements

- Linux only (requires /dev/input and uinput kernel module)
- Requires appropriate permissions to access input devices (typically root or input group)
