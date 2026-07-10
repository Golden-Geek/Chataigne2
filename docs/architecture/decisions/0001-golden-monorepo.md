# ADR 0001: One Golden monorepo

- Status: Accepted
- Date: 2026-07-10

## Decision

Golden foundations, UI packages, generated protocol, tools, and Chataigne live in one
repository with one root Rust workspace and one root JavaScript workspace. Git submodules
and embedded package copies are removed during the clean-sheet cutover.

## Consequences

Cross-layer changes become atomic. Internal packages use declared public workspace
dependencies. Repository history may be imported beneath final paths, but history does
not preserve old APIs or boundaries. The workspace must build from one clone without
submodule initialization before Phase 1 completes.
