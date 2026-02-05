# Architecture

## Overview

Remapper is organized as a Rust application with platform abstraction layers and
feature modules for config, mappings, devices, and user interfaces.

## Core Layers

- `src/core/`: shared event types, errors, and remapping engine logic
- `src/config/`: config schema, persistence, and migration support
- `src/mappings/`: mapping handlers (simple, macro, combo, conditional)
- `src/devices/`: device discovery and high-level input/output wrappers
- `src/platform/`: platform-specific backends and trait definitions
- `src/cli/`: command-line UX and workflows
- `src/gui/`: `iced` GUI flows
- `src/daemon/`: background runtime and IPC

## Data Flow

1. Input backend reads raw events from physical devices.
2. Core engine routes events through mapping handlers.
3. Output backend emits transformed events to virtual devices.
4. Config drives profile selection, mapping behavior, and runtime options.

## Platform Model

- Traits in `src/platform/traits.rs` define portable interfaces.
- Linux/Windows/macOS implementations live under `src/platform/<os>/`.
- High-level components depend on traits, not concrete platform details.

## Testing

- Unit tests are colocated with modules.
- Integration tests are under `tests/integration/`.
- Fixture configs are stored in `tests/fixtures/`.
