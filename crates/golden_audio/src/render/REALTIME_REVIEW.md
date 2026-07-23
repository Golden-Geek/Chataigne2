# Callback-owned render review

Review every change below `src/render` and `src/realtime` against this list:

- No heap allocation, reallocation, or deallocation after `RenderProcessor` construction and
  warm-up.
- No mutex, read/write lock, condition variable, blocking channel, wait, sleep, filesystem, device,
  decoder, logger, formatter, or string construction on a valid render call.
- No route, UUID, device, physical-channel, or Chataigne tree resolution in `render`.
- All buffer, channel, frame, route, voice, and analysis capacities are validated before entering
  callback-owned work.
- Render plans are immutable. Mutable gain/ramp state is preallocated separately and indexed exactly
  like the compiled plan.
- Plan, stream, asset, and buffer owners are never finally dropped by callback-owned code.
- Plan swaps acknowledge one pending plan at a time. A full return path retains ownership in a
  fixed callback slot and declines another swap until the old plan is returned.
- High-frequency parameters coalesce only inside a sequence-barrier interval. Play and stop
  barriers preserve control ordering.
- Voice assets and analysis frames move through fixed slots or bounded SPSC queues; saturation
  retains ownership and increments a preallocated counter.
- Callback entry uses `RealtimeScope` in debug/test builds. Constructors, publishers, reclaimers,
  device operations, decoders, and other control-thread APIs call `assert_not_realtime`.
- Recoverable starvation/underflow produces silence and counters, never a wait or panic.
- Invalid floating-point input is contained before device conversion; integer device conversion
  saturates and reports clipping.
- Route-kernel changes remain covered by scalar-reference equivalence and allocation/deallocation
  tests.
