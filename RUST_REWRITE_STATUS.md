# Synara Rust Rewrite

Target: fresh Rust implementation using a shared Rust core with Tauri and GPUI presentation layers.

## Execution status

The rewrite is being implemented incrementally on the `rust-rewrite` branch. The existing TypeScript/Bun/Electron implementation remains untouched as the behavioral reference.

## Current foundation

- Cargo workspace and stable Rust toolchain
- UI-independent core primitives and typed IDs
- Canonical domain entities
- Layered configuration model
- Cross-platform platform boundary
- Initial server, CLI, Tauri, and GPUI application shells

## Architecture constraints

- Durable state belongs to the Rust application core.
- Provider-native behavior is isolated behind adapters.
- State-changing operations have one authoritative orchestration path.
- Tauri and GPUI contain presentation/native-shell concerns, not domain logic.
- Existing transport compatibility and recovery semantics are preserved during migration.
- Existing implementation is not deleted until replacement behavior is validated.

## Next implementation layers

Persistence/events, runtime supervision, orchestration, process/PTY, provider adapters, ACP/MCP, Git/worktrees, HTTP/WebSocket, Tauri integration, React compatibility, GPUI, advanced integrations, cross-platform testing and release packaging.
