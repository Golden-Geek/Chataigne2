# Callback-owned render review

Review every change below `src/render` against this list:

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
- Recoverable starvation/underflow produces silence and counters, never a wait or panic.
- Invalid floating-point input is contained before device conversion; integer device conversion
  saturates and reports clipping.
- Route-kernel changes remain covered by scalar-reference equivalence and allocation/deallocation
  tests.
