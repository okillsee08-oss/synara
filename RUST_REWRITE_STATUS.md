# Synara Rust Rewrite Status

The `rust-rewrite` branch is an active replacement implementation. The TypeScript/Bun/Electron implementation on `main` remains the behavioral reference until Rust parity is validated.

## Implemented

- Rust 2024 workspace with modular core, domain, configuration and platform boundaries.
- Durable SQLite event store with WAL, migrations, event indexes and initial project/thread/message projections.
- Durable orchestration command path for projects, threads, messages, turns and approvals.
- Bounded provider runtime event channel and provider agent lifecycle boundary.
- Provider registry and process-backed adapters for Codex, Claude Agent, Cursor, Devin, Antigravity, Grok, Droid, OpenCode and Pi.
- Async ACP JSON-RPC process transport.
- Async MCP stdio JSON-RPC transport.
- Central security decisions for network, shell, filesystem and Git push.
- Workspace-confined filesystem reads and writes.
- Git repository/status and worktree boundaries.
- Cross-platform PTY terminal boundary.
- Automation/runtime/diagnostics foundations.
- HTTP API for health, negotiation, event replay, project/thread/message commands and provider discovery.
- WebSocket connection endpoint with protocol hello and ping/pong handling.
- Tauri 2 desktop shell.
- GPUI native shell.
- CLI and headless server entry points.
- Rust CI workflow for formatting, workspace compilation and tests.

## Still required for full parity

The rewrite is not yet feature-complete. Remaining work includes provider-native protocol parity and streaming ingestion, complete ACP/MCP capability negotiation and lifecycle semantics, full WebSocket snapshot/replay/fence recovery, complete persistence/projections, Git/worktree operations, terminal resize/streaming, automation persistence/recovery, browser/computer/voice implementations, migration of the existing React application, desktop supervision/native integrations, advanced Synara subsystems, cross-platform packaging, and comprehensive unit/integration/E2E acceptance coverage.

The branch must not be described as a complete replacement until those behaviors are implemented and CI passes on the resulting head.
