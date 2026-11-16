# M14/7 optional Screen sender names

**Accepted: 2026-11-16. M14/7 closes; overall92/106 (87%), M14 4/10.**

Fresh Screens now disable the real Sender name String Parameter. Shared Texture output then
uses `MapGyver - <Screen name>`. Enabling the Parameter restores its preserved custom text;
renames affect automatic mode only. The ordinary Inspector owns the checkbox and text widget.
Public configuration represents automatic mode explicitly, so FPS/quality edits preserve it.
See [ownership and protocol](../architecture/mapgyver-screen-senders.md).

The corrected common suite passes **721 tests; five existing ignored checks**, including
fresh defaults, automatic/custom names, metadata changes, history, dormant values, multiple
Screens, exact collision rejection and sparse reload. Generated TypeScript matches Rust;
Svelte checking reports zero errors/warnings. The initial common run exposed a regression:
unconfigured Fullscreen Screens were counted as display reservations. Restoring the original
nonempty-display requirement fixes it; the original failing evidence is retained.

Final review also corrected configuration ordering: requested Sender name metadata is applied
before typed Parameter validation within the same history transaction, so a dormant endpoint
cannot reject a valid destination. All122 affected tests pass afterward, including both mode
changes, one-step history/undo and generated contracts. The original721-test common run is
retained for its preceding source scope; unchanged tests were not recounted as a new full run.

The additional hosted-script test passes in automatic and explicit modes, with dormant-value
preservation and one-step undo. Strict app/library/test Clippy, both workspace formatters,
scoped Prettier, generated-contract checks and the ordinary default desktop build pass.

Intel PID44444 and NVIDIA PID15076 each pass all16 actual-app checks. The real generic
Inspector exposes the disabled Sender name switch; native Inspector renames and toggles
agree with public transport, exact SDK discovery and independent consumer pixels. Each GPU
passes ten complete red96×64 images: automatic/renamed, custom/renamed, disabled custom,
FPS round trip, two-Screen copy, automatic reload, restored dormant custom and collision recovery.
That is20 images /122,880 exact RGBA texels, with every consumer reporting retirement.

An independent owned producer temporarily claims the requested custom name. Both runs observe
the existing per-Screen preview error `0x800700B7`, with no retained SDK-suffixed sender. After
that producer exits normally, MapGyver retries and restores exact red pixels without another
authored edit. Copy retains dormant text; removal, Rectangle targeting and New withdraw the
old SDK names. Final New records one completed old native-owner retirement per GPU. Its
replacement still renders the main preview, with retiring0; this is not an idle/zero-allocation,
peak-growth, timing or sustained-performance claim.

Two failed fixture attempts remain recorded: the first queried native control during startup
navigation; the second looked for an output collision in the global renderer error. Source
inspection shows that existing output errors belong to the per-Screen preview event. The final
fixture waits for the actual Inspector and verifies that original event and HRESULT, keeping
the original deadlines and pixel/retirement requirements. No product error path was weakened.

Commands, binary/log identities and exact receipt hashes are in the
[acceptance receipt](evidence/mapgyver-m14-sender-names-2026-11-16.json).
M14/6 physical/native-input cases, Video/performance and final qualification remain open.
