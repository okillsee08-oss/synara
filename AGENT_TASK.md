# Agent Task: Tests / Verification

Base: V1-Proto

Own regression coverage and integration verification. Identify high-risk gaps in lifecycle, persistence, streaming, cancellation, recovery, provider events, and cross-package contracts. Prefer small deterministic tests. Do not weaken tests to make failures pass. Run the narrowest useful tests first, then broaden when stable.

Completion: tests that catch real regressions, with exact commands/results.
