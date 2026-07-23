# Architecture

This section explains ownership and dependency direction. Start with the
[top-level architecture](../../ARCHITECTURE.md), then open only the subsystem you are changing.

## Foundation

- [Application contracts](application-contracts.md): reusable application and host seams.
- [Foundation ownership](foundations.md): identities, values, parameters, and contexts.
- [Golden Audio](golden-audio.md): reusable audio ownership, stable identities, render plans,
  callback safety, clocks, and device recovery.
- [Golden Graph](graph-foundation.md): graph documents, transactions, revisions, and presentation.
- [Hosts](hosts.md): desktop, headless/browser, transport, and native-dialog ownership.
- [UI protocol](ui-protocol.md): Rust DTO source of truth and generated TypeScript.

## Chataigne systems

- [Alchemist runtime](alchemist-runtime.md): formulas, processors, conditions, inputs, filters,
  outputs, lane memory, and diagnostics.
- [State machine and Alchemist processing](statecharts-conditions-processors.md): how Chataigne's
  state/transition runtime consumes Alchemist processors.

## Placement rule

Reusable node/runtime infrastructure belongs under `crates/golden_core/`. Generic graph document
and canvas behavior belongs in `crates/golden_graph/` or `packages/golden-graph-ui/`.
Chataigne product behavior belongs under `apps/chataigne/systems/`, with app-engine integration in
each system's `integration/` folder and product UI under `apps/chataigne/ui/`.

For exact paths, see [Repository Layout](../reference/repository-layout.md).
