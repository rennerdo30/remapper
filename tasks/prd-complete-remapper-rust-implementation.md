# PRD: Complete Remapper Rust Implementation

## Overview
Complete all unfinished features, stubs, and TODOs in the Rust rewrite of Remapper. This includes fixing the GUI profile start functionality, implementing daemon config reload, and addressing platform-specific gaps on macOS and Windows using third-party libraries where possible.

## Goals
- Complete all TODO items and stub implementations in the Rust codebase
- Ensure GUI can actually start/stop remapping profiles
- Implement proper daemon SIGHUP config reload (stop remaps → reload → restart)
- Address macOS and Windows platform limitations using available libraries
- Achieve feature parity across Linux, macOS, and Windows where technically feasible

## Quality Gates

These commands must pass for every user story:
- `cargo build --all-targets` - Build all targets
- `cargo test` - Run all tests
- `cargo clippy -- -D warnings` - Strict linting with no warnings

## User Stories

### US-001: Implement GUI Profile Start with Background Thread
As a user, I want to start a remapping profile from the GUI so that I can remap inputs without blocking the interface.

**Acceptance Criteria:**
- [ ] "Start Profile" button spawns remapping engine in background thread
- [ ] GUI remains responsive while remapping is active
- [ ] Status indicator shows when profile is running
- [ ] "Stop Profile" button cleanly terminates the background thread
- [ ] Error handling displays failures to user (e.g., permission denied)

### US-002: Implement GUI Daemon Mode Support
As a user, I want the option to start remapping via the daemon so that remapping persists after closing the GUI.

**Acceptance Criteria:**
- [ ] Settings option to choose "background thread" vs "daemon" mode
- [ ] When daemon mode selected, GUI spawns/connects to daemon process
- [ ] GUI can start/stop profiles through daemon IPC
- [ ] GUI shows daemon connection status
- [ ] Graceful handling when daemon is unavailable

### US-003: Implement Daemon SIGHUP Config Reload
As a user, I want to send SIGHUP to the daemon to reload configuration so that I can update remaps without restarting the service.

**Acceptance Criteria:**
- [ ] SIGHUP handler stops all active remapping sessions
- [ ] Configuration file is re-read from disk
- [ ] All previously running profiles are restarted with new config
- [ ] Errors during reload are logged (don't crash daemon)
- [ ] If config is invalid, keep old config and log error

### US-004: Implement macOS Scroll Event Output
As a user on macOS, I want scroll wheel remapping to work so that I can remap scroll inputs.

**Acceptance Criteria:**
- [ ] Research and implement scroll event output using CGEvent or IOKit
- [ ] Support both vertical and horizontal scroll
- [ ] Scroll events work in all applications
- [ ] If not possible, document limitation clearly in code and user docs

### US-005: Implement macOS Gamepad Output Using Library
As a user on macOS, I want gamepad output remapping so that I can remap keyboard/mouse to gamepad buttons.

**Acceptance Criteria:**
- [ ] Evaluate `gilrs` or similar library for gamepad output on macOS
- [ ] If library supports output, implement virtual gamepad creation
- [ ] If no library available, investigate foohid or DriverKit alternatives
- [ ] Document any permission requirements (accessibility, input monitoring)
- [ ] If not feasible, clearly document limitation and remove misleading code paths

### US-006: Implement macOS Full Keyboard/Mouse Input
As a user on macOS, I want to capture keyboard and mouse input so that I can create remaps from any input device.

**Acceptance Criteria:**
- [ ] Implement keyboard capture using IOKit HID Manager or CGEvent tap
- [ ] Implement mouse capture (buttons and movement)
- [ ] Handle accessibility permission requirements gracefully
- [ ] Prompt user for permissions if not granted
- [ ] Support both global capture and per-device capture where possible

### US-007: Implement Windows Keyboard/Mouse Input via Raw Input API
As a user on Windows, I want keyboard and mouse input capture so that I can create remaps from these devices.

**Acceptance Criteria:**
- [ ] Implement Raw Input API for keyboard capture
- [ ] Implement Raw Input API for mouse capture
- [ ] Support capturing from specific devices (not just global)
- [ ] Handle Windows permission/elevation requirements
- [ ] Evaluate `winit` or `windows-rs` crate for implementation

### US-008: Fix Windows Gamepad Output Sync Trait Design
As a developer, I want the Windows gamepad output to properly implement the sync trait so that the code compiles without workarounds.

**Acceptance Criteria:**
- [ ] Analyze why `ViGEmClient` doesn't implement Sync
- [ ] Implement proper thread-safe wrapper or use interior mutability
- [ ] Remove placeholder `unsafe impl Sync` if present
- [ ] Ensure gamepad output works correctly in multi-threaded context
- [ ] Add tests for concurrent gamepad output

### US-009: Clean Up Empty Placeholder Files
As a developer, I want unused placeholder files removed so that the codebase is clean and maintainable.

**Acceptance Criteria:**
- [ ] Remove or implement `src/gui/main_view.rs` 
- [ ] If main_view.rs is needed, implement actual functionality
- [ ] If not needed, remove file and any references to it
- [ ] Ensure no other empty/placeholder files exist

### US-010: Add Cross-Platform Input Device Enumeration
As a user, I want to see available input devices on any platform so that I can select which device to remap.

**Acceptance Criteria:**
- [ ] Linux: enumerate via /dev/input (already working)
- [ ] macOS: enumerate HID devices via IOKit
- [ ] Windows: enumerate via Raw Input or SetupAPI
- [ ] Unified device list format across platforms
- [ ] Show device name, type (keyboard/mouse/gamepad), and ID

## Functional Requirements
- FR-1: GUI must support both background thread and daemon execution modes
- FR-2: Daemon must reload config on SIGHUP without crashing
- FR-3: All platforms must support keyboard input capture
- FR-4: All platforms must support mouse input capture  
- FR-5: All platforms must support gamepad input capture (via gilrs or similar)
- FR-6: Keyboard/mouse output must work on all platforms
- FR-7: Gamepad output must work on Linux and Windows (macOS best-effort)
- FR-8: Platform limitations must be clearly documented in code

## Non-Goals
- Python codebase maintenance or improvements (deprecated)
- Custom DriverKit development for macOS (too complex)
- Kernel driver development for any platform
- Mobile platform support (iOS/Android)
- Supporting input devices beyond keyboard/mouse/gamepad

## Technical Considerations
- Use `gilrs` crate for cross-platform gamepad support where possible
- Use `windows-rs` for Windows API access
- Use `core-foundation` and `core-graphics` crates for macOS
- IOKit HID Manager required for macOS device enumeration
- Raw Input API required for Windows keyboard/mouse capture
- Consider `tokio` for async daemon implementation
- Thread safety is critical for background remapping

## Success Metrics
- All TODO comments in Rust code are resolved
- GUI can start/stop profiles without blocking
- Daemon properly reloads on SIGHUP
- Keyboard/mouse input works on macOS and Windows
- All quality gate commands pass
- No clippy warnings

## Open Questions
- Is there a maintained Rust library for virtual gamepad output on macOS?
- Should we use CGEvent taps or IOKit for macOS keyboard capture?
- What elevation/permissions are needed on Windows for Raw Input?
- Should daemon use Unix sockets, TCP, or named pipes for IPC?