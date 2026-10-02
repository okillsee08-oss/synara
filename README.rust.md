# Synara Rust Rewrite

This branch is the clean Rust architecture for Synara. It keeps the existing application intact while replacing its implementation behind stable domain, event, provider and transport boundaries.

## Build

Install stable Rust and run:

    cargo check --workspace
    cargo test --workspace
    cargo run -p synara-server

The server defaults to 127.0.0.1:3210 and supports SYNARA_ADDR for binding configuration.

## Architecture

Core/domain -> events/database -> orchestrator -> runtime/providers/process/Git/files -> API/transport -> Tauri/GPUI/CLI.

Provider-native protocols are isolated behind adapters, durable state is persisted in SQLite, and event sequence numbers provide replay foundations for reconnecting clients.
