# Screen output cadence

MapGyver owns authored output policy in each Screen's real Golden Parameters:
`frame_rate_mode` selects **Follow display** or **Explicit**; `frames_per_second` is the
remembered finite value from 1 to 1000 FPS. The generic Inspector shows FPS in Explicit mode.
Screen operations, scripts, generic Parameter edits, history, copying and sparse projects use
these same children. `ScreenDefinition.frame_rate` is their validated projection; Rust generates
the TypeScript contract. Explicit decimal rates resolve to a rational frequency at micro-FPS
precision. Fractional display rates retain the OS numerator and denominator exactly.

`screens/frame_rate.rs` resolves policy from current physical routing. Fullscreen follows the
resolved monitor, including the existing recovery assignment, while retaining authored display
identity. Rectangle follows the display with the largest physical overlap (primary, then stable
identity break ties). Shared Texture has no physical display; it uses 60 FPS in Follow display
mode. Unknown refresh or an off-desktop rectangle also uses 60 FPS. These fallback rules do not
rewrite authored settings. The desktop display catalog carries the optional OS desktop refresh
rate, queried on the preparation worker through Golden's bounded display enumeration.

Golden Video owns reusable mechanics. `RuntimeConfig.additional_frame_rates` adds at most 32
independent absolute clocks to the primary content clock. Their deadline union wakes the worker;
50 FPS and fractional rates are not quantized onto a faster consumer's grid. Stalls coalesce
missed slots without catch-up loops. Frame observations identify the selected clock's rate and
ordinal. `D3d11Config.output_rates` admits unique bound consumer/rate pairs and automatically
contributes their clocks. Native endpoint pacing gates conversion and delivery before admission,
including cached presentation redraws. The render core admits no GPU batch or completion slot
when no consumer is due; cached redraws consider only dirty consumers. It retains phase through show seek/pause and starts a new
phase for a replacement monotonic authority or device lifetime.

MapGyver's native host assigns its independent preview consumer 60 FPS and assigns each Screen
its resolved output rate. Shared source evaluation remains one runtime, rather than one decoder
or renderer per Screen. Rendering shared content runs at the deadline union; selecting a lower
output rate limits delivery, not all upstream work. A rate change currently uses the same complete
GPU retirement/rebinding path as other immutable native configuration changes, preserving physical
window leases and the pinned adapter. Display refresh changes invalidate the display capture and
resolve policy again. This is a configuration change, not a per-frame recreation.

API-accepted output counts establish delivery behavior only. They do not certify physical
scanout, manipulation smoothness, video/overlay alignment, latency or production GPU/cadence
budgets. Those requirements remain in M13/4 and the existing qualification return queue.

Windows frequency metadata uses
[`DISPLAYCONFIG_PATH_TARGET_INFO.refreshRate`](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_path_target_info),
which describes the desktop refresh frequency. It is not a measurement of presented frames.
