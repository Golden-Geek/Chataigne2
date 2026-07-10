# ADR 0005: Clean break with no production legacy path

- Status: Accepted
- Date: 2026-07-10

## Decision

Functional behavior is preserved, but internal APIs, crate/package boundaries, protocols,
IDs, and unreleased persistence schemas are not. Old repositories are read-only references
while the new implementation is built. Temporary dual execution is allowed only in tests
for semantic comparison.

## Consequences

There are no permanent adapters between old and new architectures. Valuable development
fixtures may use a disposable offline converter, which is deleted after conversion. Phase 9
removes every old runtime, protocol, graph store, Alchemist graph type, feature flag,
submodule, and obsolete architecture document after parity and performance gates pass.
