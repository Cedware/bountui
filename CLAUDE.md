# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

bountui is a terminal UI (TUI) application for HashiCorp Boundary, built with Rust using ratatui for the terminal interface and tokio for async runtime.

## Build Commands

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run
cargo run

# Run tests
cargo test

# Run a specific test
cargo test <test_name>

# Run tests in a specific module
cargo test <module_path>::tests
```

## Architecture

### Module Structure

- **`boundary/`** - Boundary API client layer
  - `client/cli/` - CLI wrapper around the `boundary` binary (executes commands like `boundary scopes list -format json`)
  - `client/mod.rs` - `ApiClient` trait defining the interface (get_scopes, get_targets, get_sessions, connect, etc.)
  - `models.rs` - Data structures for Boundary entities (Scope, Target, Session, ConnectResponse)

- **`bountui/`** - UI layer
  - `mod.rs` - Main `BountuiApp` struct with event loop, message handling, and page navigation
  - `components/` - UI components (table pages for scopes/targets/sessions, dialogs, toaster)
  - `widgets/` - Reusable ratatui widgets (Alert, Toast, ConnectionResultDialog)
  - `connection_manager.rs` - Manages active Boundary connections

- **`util/`** - Utilities (clipboard access)

### Key Patterns

**Message-based architecture**: The app uses a message passing pattern where `Message` enum variants trigger state changes. Components send messages via `tokio::sync::mpsc` channels, and `BountuiApp::handle_message()` processes them.

**Page navigation**: `Page` enum represents different views (Scopes, Targets, TargetSessions, UserSessions). Navigation history is maintained in a stack for back navigation.

**Trait-based abstraction**: `ApiClient` trait abstracts the Boundary CLI, enabling mocking in tests. `BoundaryConnectionHandle` trait abstracts connection lifecycle.

### Testing

Tests use `mockall` for mocking traits. The `MockApiClient` and `MockConnectionManager` are auto-generated. Test helpers exist in `src/mock.rs`.

## Environment

- Requires `boundary` CLI in PATH
- Log level controlled via `LOG_LEVEL` env var (default: info)
- Logs stored in `~/.local/share/bountui/logs/` (Linux/Mac) or `%APPDATA%\bountui\logs\` (Windows)
- User inputs persisted to `~/.bountui/user_inputs.json`
