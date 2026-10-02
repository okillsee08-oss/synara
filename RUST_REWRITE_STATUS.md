# Synara Rust Rewrite

The rust-rewrite branch contains the independent Rust implementation while main remains the behavioral reference.

## Implemented
- Cargo workspace with dedicated core/domain/config/platform crates.
- Durable event model and SQLite persistence with migrations and sequence replay.
- Orchestration command path for project/thread/message/turn/tool approval lifecycle events.
- Tokio runtime primitives and cancellation supervision.
- Cross-platform process abstraction and terminal boundary.
- Provider adapter registry with built-in provider metadata and process-backed adapters for Codex, Claude Agent, Cursor, Devin, Antigravity, Grok, Droid, OpenCode and Pi.
- ACP and MCP protocol data boundaries.
- Central security policy boundary for network, shell and filesystem operations.
- Workspace-confined filesystem access.
- Git repository/status and worktree boundaries.
- Automation scheduler primitives.
- Axum server/API foundation and typed transport/event-bus primitives.
- Diagnostics, browser, computer-use and voice extension boundaries.
- CLI command surface and runnable HTTP server.

## Compatibility direction
The Rust core is authoritative for durable orchestration. Provider-native protocols remain behind adapters; transport and UI are replaceable. The existing TypeScript/Bun/Electron tree remains intact until equivalent behavior is validated.

## Verification
The branch is structured as a Cargo workspace and includes the implementation layers needed for migration. Full dependency compilation and end-to-end parity validation requires a Cargo-enabled checkout or CI runner; GitHub file operations alone cannot execute Cargo locally.
