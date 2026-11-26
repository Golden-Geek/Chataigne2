# Architecture

Start with the [top-level architecture](../../ARCHITECTURE.md). The [Chataigne plan](../plan/Chataigne2_Codex_Implementation_Plan.md)
and [status](../progress/implementation-status.md) track product implementation.

[Golden Core's architecture](https://github.com/Golden-Geek/GoldenCore/tree/main/docs/architecture)
covers reusable foundations, runtime, persistence, transports and workbench.
[MapGyver2's architecture](https://github.com/Golden-Geek/MapGyver2/blob/main/docs/architecture/mapgyver.md)
covers the video mapping product.

## Chataigne systems

- [OSC native workers](osc-runtime.md): bounded socket ownership, readiness, accepted
  command/input prefixes, interface catalogs and passive service.
- [MIDI workers and delivery](midi-runtime.md): native discovery/input/output, prepared catalog lifetimes,
  prepared input, completion receipts, feedback provenance, outgoing batches and passive deadlines.
- [MQTT ownership](mqtt-runtime.md): bounded native protocol ownership, retained publish outcomes,
  reconnect/retirement and locally qualified passive app service.
- [DMX ownership](dmx-runtime.md): native setup/cleanup, bounded frames, completion receipts and
  passive actor service; P07 owns stream correctness.
- [System module ownership](https://github.com/Golden-Geek/GoldenCore/blob/main/docs/architecture/system-runtime.md): captured OS commands, bounded metrics/native
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
- [Gamepad worker ownership](gamepad-runtime.md): bounded Gilrs ownership, prepared calibration
  and callbacks, retained choices, configuration fences and passive recovery.
- [Kinect2 worker ownership](kinect2-runtime.md): bounded sensor/reader lifetimes, prepared skeletons
  and SDK-time hand metrics, reference-space revisions, guarded projection and passive recovery.
- [Stream Deck service ownership](streamdeck-runtime.md): native ownership, bounded
  page preparation, retained input routes and latest visual feedback.
- [Timed generators](generators.md): Signals/Metronomes admission, elapsed time, bounded
  publication, passive service and worker lifetime ownership.
- [Spatializer service](spatializer-runtime.md): admitted geometry preparation, bounded matrix
  publication, deterministic identities and passive recovery.
- [Alchemist runtime](alchemist-runtime.md): formulas, processors, conditions, inputs, filters,
  outputs, lane memory, and diagnostics.
- [Built-in Mapping](builtin-mapping.md): ordered tuple stages, bindings, flow, and ownership
  contracts for the Inputs / Filters / Outputs Formula.
- [State machine and Alchemist processing](statecharts-conditions-processors.md): how Chataigne's
  state/transition runtime consumes Alchemist processors.

Chataigne systems own product behavior and their `integration/` modules connect it to the engine.
Product UI lives under `apps/chataigne/ui/`. See the [repository layout](../reference/repository-layout.md)
for exact paths.
