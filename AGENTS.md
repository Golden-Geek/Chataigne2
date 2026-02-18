# AGENTS.md

## Mission
This repository hosts active development for `golden_core` and `Chataigne2`.

The priority is to build a clean, correct, and efficient long-term foundation.
Backward compatibility is not a goal during this phase.

## Development Stance
- Breaking changes are allowed when they improve architecture, clarity, safety, or performance.
- Prefer removing flawed or legacy patterns instead of layering temporary compatibility shims.
- Challenge existing proposals when a cleaner or more robust approach exists.
- Optimize for maintainability and correctness first, then ergonomics and speed.

## Rust Engineering Standards
- Follow Rust best practices consistently (`idiomatic ownership`, `type-driven design`, `explicit error handling`, and `zero-cost abstractions`).
- Favor compile-time guarantees over runtime patching.
- Keep modules cohesive, APIs minimal, and invariants explicit.
- Avoid hidden side effects and unnecessary indirection.
- Treat warnings, clippy findings, and questionable patterns as signals to improve design.

## Svelte Standards
- Implement only Svelte 5 code and runes; avoid legacy patterns or manual reactivity.
- Use only relative units (em, rem, %, vh, vw) for sizing and spacing; no fixed pixel values.
- Leverage runes for reactive state management with a single source of truth; components subscribe and react to updates automatically.
- Keep component logic minimal; let runes propagate changes throughout the component tree.
- Only use $derived.by() when $derived() is not enough

## Decision Rules
- If two approaches work, choose the one with the simpler and more defensible architecture.
- If a proposal adds complexity without strong value, discard it.
- If a refactor yields a cleaner core, prefer it even when it requires migration work.
- Document intent where design tradeoffs are non-obvious.
- Whenever making modifications or adding things, make sure to clean obsolete code, only keep things we actually will use in this context

## Quality Bar
- Code should be production-oriented even during rapid iteration.
- Every change should leave the codebase cleaner than it was before.
- Temporary hacks are acceptable only when explicitly scoped and scheduled for removal.
