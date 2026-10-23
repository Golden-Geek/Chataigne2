# Architecture

This section explains ownership and dependency direction. Start with the
[top-level architecture](../../ARCHITECTURE.md), then open only the subsystem you are changing.

For execution order use the single [implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md)
and [current status](../progress/implementation-status.md). Historical implementation narratives
and superseded plans are [archived](../archive/2026-10-18/README.md).

## Foundation

- [Application contracts](application-contracts.md): reusable application/host seams and parameter event attribution.
- [Foundation ownership](foundations.md): identities, values, parameters, and contexts.
- [Golden Audio](golden-audio.md): reusable audio ownership, stable identities, render plans,
  callback safety, clocks, and device recovery.
- [Golden Graph](graph-foundation.md): graph documents, transactions, revisions, and presentation.
- [Hosts](hosts.md): desktop, headless/browser, transport, and native-dialog ownership.
- [Persistence transactions](persistence.md): save ordering, destination identity, recovery, and
  project-generation fencing.
- [UI protocol](ui-protocol.md): Rust DTO source of truth and generated TypeScript.

## Chataigne systems

- [OSC native workers](osc-runtime.md): bounded socket ownership, readiness, accepted
  command/input prefixes, interface catalogs and passive service.
- [MIDI workers and delivery](midi-runtime.md): native discovery/input/output, prepared catalog lifetimes,
  prepared input, completion receipts, feedback provenance, outgoing batches and passive deadlines.
- [MQTT ownership](mqtt-runtime.md): bounded native protocol ownership, retained publish outcomes,
  reconnect/retirement and locally qualified passive app service.
- [DMX ownership](dmx-runtime.md): native setup/cleanup, bounded frames, completion receipts and
  passive actor service; P07 owns stream correctness.
- [System module ownership](system-runtime.md): captured OS commands, bounded metrics/native
  lifetimes, completion outcomes and passive App Control/OS service.
- [Buttplug worker ownership](buttplug-runtime.md): bounded protocol workers, native lifetime
  slots, retained command outcomes, passive service and local C07 qualification.
- [Joy-Con worker ownership](joycon-runtime.md): bounded native lifetime, command/input admission,
  canonical command/script capture, retained outcomes, rumble maintenance under pressure and passive actor service.
- [Keyboard worker ownership](keyboard-runtime.md): bounded input/output lifetimes, canonical
  command/script capture, prepared input callbacks and shared choices, Windows input registration,
  passive input/output service and queued projection guards.
- [Mouse worker ownership](mouse-runtime.md): bounded input/output lifetimes, canonical command/script
  capture, prepared input publication, shared Windows registration, common Keyboard/Mouse output
  ordering and passive service.
- [Ultraleap worker ownership](ultraleap-runtime.md): bounded SDK lifetimes, prepared tracking
  publication, ordered status boundaries, queued projection guards and passive recovery.
- [Timed generators](generators.md): Signals/Metronomes admission, elapsed time, bounded
  publication, passive service and worker lifetime ownership.
- [Alchemist runtime](alchemist-runtime.md): formulas, processors, conditions, inputs, filters,
  outputs, lane memory, and diagnostics.
- [Built-in Mapping](builtin-mapping.md): ordered tuple stages, bindings, flow, and ownership
  contracts for the Inputs / Filters / Outputs Formula.
- [State machine and Alchemist processing](statecharts-conditions-processors.md): how Chataigne's
  state/transition runtime consumes Alchemist processors.

## Placement rule

Reusable node/runtime infrastructure belongs under `crates/golden_core/`. Generic graph document
and canvas behavior belongs in `crates/golden_graph/` or `packages/golden-graph-ui/`.
Chataigne product behavior belongs under `apps/chataigne/systems/`, with app-engine integration in
each system's `integration/` folder and product UI under `apps/chataigne/ui/`.

For exact paths, see [Repository Layout](../reference/repository-layout.md).
