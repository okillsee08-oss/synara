# Agent Task: Runtime / Process

Base: V1-Proto

Own provider runtime, process lifecycle, cancellation, teardown, streaming/reconnect handling, and recovery boundaries. Preserve platform/process abstraction boundaries and never claim cleanup without verification. Inspect existing provider contracts before changing APIs. Run targeted Rust tests and the Windows runtime boundary check where applicable.

Completion: focused commits plus explicit verification and unresolved platform limitations.
