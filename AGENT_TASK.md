# Agent Task: Rust Core

Base: V1-Proto

Own the Rust domain/core rewrite. Inspect existing crates before adding abstractions. Priorities: domain invariants, typed IDs/errors, serialization contracts, deterministic state transitions, and focused unit tests. Do not modify web UI or provider-specific process code unless required by a core contract. Verify with cargo fmt --all -- --check and targeted cargo tests.

Completion: commit only focused Rust-core changes; report tests run and any dependency/blocking contract.
