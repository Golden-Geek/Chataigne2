# ADR 0001: Form One Golden Monorepo from the Complete Working Product

- Status: Accepted
- Date: 2026-07-11

## Context

Chataigne currently consumes `golden_core`, `golden_alchemist_core`, `golden_ui`, and
`golden_alchemist_ui` as four Git submodules. The boundaries that must evolve together cannot be
changed atomically, while the failed rewrite demonstrated that creating clean empty packages first
can discard the actual product.

## Decision

Create one canonical Golden monorepo with one root Rust workspace and one root JavaScript workspace.
Chataigne is an application within it. Form the monorepo by importing history and the complete
contents of the exact recorded baseline revisions, including the application, UI, modules, formulas,
assets, fixtures, desktop host, platform resources, and packaging.

The imported Chataigne application must build and launch before foundational extraction begins.
Applications use public crate/package APIs, reusable Golden packages remain app-agnostic, and cyclic
dependencies or private filesystem imports are forbidden.

Phase 1A is mechanical. It retains baseline toolchain, package-manager, framework, dependency, and
lock versions except for unavoidable path/source rewrites. Modernization occurs in separate
product-gated Phase 1B supercommits.

## Consequences

- Rust, TypeScript, generated protocol, tests, docs, and consumers can change atomically.
- Submodules cease to be required for the active workspace, while their immutable repositories and
  commits remain comparison references until final qualification.
- Empty future-package scaffolding does not count as migration.
- Root `cargo run`, `watch`, and `cargo run -- --dev` remain stable product contracts during moves.
- The older separate-SDK-repository direction in `docs/repo-transition-plan.md` is superseded by this
  decision for the active migration.

## Compliance

The Phase 1A gate requires one clone to launch the complete product, match approved manifests, load
and save representative projects, support local Tauri and remote-browser workflows, and require no
submodule initialization.
