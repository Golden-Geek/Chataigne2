# MapGyver native rendering research

Research date: 2026-10-27; functional route decision updated 2026-10-28. The user approved
ordinary Tauri/Wry + D3D11 + pinned `spout2-sys =0.1.1` for the thin M02 shell, based on
[scoped native evidence](../progress/mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28).
This is not a full performance/display qualification. See [ownership and product contracts](mapgyver.md)
and the [implementation plan](../plan/MapGyver_Implementation_Plan.md).

## Physical GPU identity and recovery

Golden's native identity boundary distinguishes `PhysicalAdapterId` (an exact Windows PnP
device instance) from `AdapterId` (the current DXGI LUID). Preferences persist the former;
runtime status, texture sharing and memory observations retain the latter. Windows can
[replace a LUID when an adapter stops/starts](https://learn.microsoft.com/en-us/uwp/api/windows.devices.display.core.displayadapter.id),
while the [device instance identifies the persistent device](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/device-instance-ids).
The read-only mapping uses `DisplayConfigGetDeviceInfo` for the adapter interface and
Configuration Manager for its device instance; it does not parse interface paths or infer
identity from labels, vendor/model, display ownership or enumeration order. Failed identity
queries remain explicit HRESULTs, and duplicate physical matches are rejected.

The native backend pins that physical binding before first allocation. Recreation resolves
only its current LUID, preserving the existing missing-device retry and no-fallback behavior.
MapGyver's host catalog refreshes the current coordinate for this same binding, and File
workers retire/rebind under the existing shared storage budget. Old images are never retagged
for a replacement device: native import still requires the exact current LUID. Authored GPU
choice does not change during recovery. Earlier persisted `dxgi:` choices are historical
runtime tokens and remain unavailable; there is no name-based conversion or data migration.
Native qualification records both original/current coordinates and independently checks the
physical binding before receiving recovered pixels. Full recovery qualification remains in
the active M10 ledger until the original fixture passes.

## Transparent WebView over native rendering

The desired "hole" is a transparent area in the HTML/WebView revealing a native GPU-rendered
surface underneath. Opaque workbench chrome and DOM input controls remain above it. Wry's official
[GPU example](https://github.com/tauri-apps/wry/blob/dev/examples/wgpu.rs) demonstrates native
rendering behind a transparent child WebView. On Windows it disables child clipping and enables
WebView transparency. The example is also present in the locally installed, locked Wry 0.55.1.
The checkout locks Tauri 2.11.5 and tauri-runtime-wry 2.11.4; the native host constrains Tauri
updates to the validated 2.11 minor while using platform types. The upstream moving example is
supporting evidence; the packaged probe must run against the repository's actual dependency lock.

Microsoft documents that transparent web content and a transparent default background reveal
the host's content. The default background accepts opaque or fully transparent alpha, not an
arbitrary intermediate value. [WebView2 background API](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller2)

Coollab is useful visual-product inspiration, but its current source selects OpenGL and exposes
native ImGui windows/menus. It is not evidence for this WebView implementation.
[Coollab build](https://github.com/Coollab-Art/Coollab/blob/main/CMakeLists.txt),
[Coollab app interface](https://github.com/Coollab-Art/Coollab/blob/main/src/App.h).

Golden's desktop host owns its Tauri builder/setup. Its optional
[`NativePresentation<R>` extension](../../crates/golden_core/hosts/desktop/src/native_presentation/mod.rs)
constructs a transparent main WebView with native underlay clipping, prepares on the UI thread
and runs an owned worker task. Exit preserves the HWND until worker retirement; joining happens
after the event loop returns. Default launches keep the ordinary opaque WebView, and headless
launches never prepare presentation. Renderer/preview/output policy stays with M03/M05 instead
of creating a second app-owned bootstrap. Tauri exposes public native window and platform-WebView access; its platform
API documentation advises pinning at least the minor version when using underlying types.
[Tauri window API](https://docs.rs/tauri/latest/x86_64-pc-windows-msvc/tauri/window/struct.Window.html),
[Tauri platform access](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/webview/mod.rs).

Physical output-window ownership is separate from GPU-session ownership. Golden desktop's
`NativeOutputWindow::set_physical_bounds` sends a bounded value-only command to the owning
per-monitor-v2 thread, preserves the visible HWND and verifies the actual physical client bounds.
MapGyver's preparation host retains window leases by project/Screen identity. Position and
same-size display-policy changes preserve the GPU binding/session; Escape uses the current
authored target. A changed backing extent requires a new admitted immutable GPU configuration:
the old session completes actual GPU and Picture retirement before replacement, while the
same window lease remains visible. Removed/disabled outputs and project replacements release
the window only after all renderer leases retire. This conservative resize path rebuilds the
renderer session, including other native bindings; it does not claim uninterrupted Spout
identity or timing qualification across backing-size changes.
Display allocation has an explicit pending handoff: a stale authored cohort or changed native
catalog retains the previous window until current metadata commits. A revision wakes preparation
even when the resulting allocation is identical. A committed missing target still withdraws the
output and releases its window after retirement; pending metadata is never mistaken for removal.

Editor decorations belong to the same native presentation as the image. Golden Video owns
bounded Quad/Circle metadata, host-authenticated physical scale and aperture clipping, checked
live placement and premultiplied linear drawing. Placement and decorations are sampled once
per GPU completion token, so independent WebView/GPU presentation cannot separate their paint
during navigation. Preview protocol v3 carries metadata only. Each view declares its fixed
decoration capacity before plan admission; ordinary previews and backdrop placements reserve
zero. Live updates cannot exceed the reserved capacity or bypass retained-input accounting.

MapGyver supplies Screen/Source geometry, styles and opaque local control identities, including
selected, hovered, disabled, invalid, crop and pointer presentation. DOM controls retain
accessibility and input; Golden's
preview component suppresses their marked paint only while an installed native view supplies
decorations. The public viewport `nativeFrame` hook also withdraws the moving DOM outline and
surround mask while native paint owns the black canvas and image frame; this permits controls
outside the image and prevents the WebView aperture from clipping an independently presented
image. Browser/unavailable previews keep their DOM paint. This rendering mechanism does
not by itself establish the retained native interaction, physical alignment or latency gates.

Golden Video retains desired, sampled and output-accepted placements independently. Only the
bound output accepting the exact sampled completion token promotes pointer geometry; rendering
alone, a busy output or another output's acceptance cannot do so. A pending replacement retains
the last accepted image/input. Source, session, viewport and device changes fence obsolete input.
The read-only pointer query returns the accepted full-image projection and topmost opaque hit;
captured drags retain coordinates outside the clipped aperture without acquiring new hits.
Golden preview UI coalesces event-driven queries to one in flight, one latest move and one
terminal event. It retargets accessible DOM controls and projects coordinates into their current
image rectangle, with logical control capture backed by real viewport pointer capture. Escape,
blur, disposal and document/source replacement cancel pending admission. Ordinary DOM fallback
uses the same public capture helper; navigation toolbar controls remain independent.

Recommended first probe: D3D11 rendering below the existing ordinary Tauri/Wry WebView, following
the layering pattern in Wry's wgpu example. The example proves the pattern, not this exact
D3D11/Tauri integration. Use one underlay per editor window and scissored preview regions.
Svelte owns pointer capture and dispatch; installed decorated previews use output-accepted native
hit testing. Visual transparency alone does not imply native input routing.

If ordinary hosting cannot satisfy layering/alignment, investigate an explicit DirectComposition
host provider. A WebView2 composition controller is a different creation/hosting path with input,
cursor/focus and accessibility obligations; it is not a cast of the current ordinary controller.
[Composition controller](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2compositioncontroller),
[composition target layering](https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-idcompositiondesktopdevice-createtargetforhwnd).

Layout registration must account for resize, scroll, docking, tab visibility, zoom and monitor/DPI
changes. WebView2 distinguishes raw-pixel bounds from rasterization-scale bounds; text scaling
can also contribute. Convert typed CSS-client geometry once in the host, using actual WebView
and window metrics. Native presentation and HTML updates are asynchronous, so measure alignment
during movement instead of promising frame-atomic updates.
[WebView2 scaling](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller3).

WebView2 objects require their UI STA thread. GPU work, frame pacing, resource waits and cleanup
must not block it. [WebView2 threading model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/threading-model).

## D3D11 and Spout

Golden's copy-only input workers use explicit D3D11 fences (`ID3D11Device5` /
`ID3D11DeviceContext4`) to certify source GPU completion before reuse or retirement.
This path must sustain moving content with GPU diagnostics disabled; timestamp collection
previously hid a production completion-query stall. Renderer/output queries and the ordinary
native underlay are separate owners. See the [cadence repair evidence](../progress/mapgyver-shared-texture-cadence.md).

Upstream Spout exposes `spoutDX` with D3D11 texture APIs, sender size/format updates, connection
state, frame information and adapter selection/discovery. Use its supported SDK rather than
reimplementing sharing/discovery. [SpoutDX API](https://github.com/leadedge/Spout2/blob/master/SPOUTSDK/SpoutDirectX/SpoutDX/SpoutDX.h).

wgpu currently supports D3D12/Vulkan/Metal/GL/WebGPU, without a D3D11 backend. Choosing it would
still require a proven Spout interop path. D3D11On12 is supported but adds wrapped-resource
acquire/release, flushing, state/lifetime coordination and overhead. These are reasons to start
with D3D11 for this Windows-only scope, not claims that another backend cannot work.
[wgpu backends](https://docs.rs/wgpu/latest/wgpu/enum.Backend.html),
[Microsoft D3D11On12](https://learn.microsoft.com/en-us/windows/win32/direct3d12/direct3d-11-on-12).

| Candidate | Source-backed capability | Selection gate |
| --- | --- | --- |
| [`spout2-rs`](https://docs.rs/crate/spout2-rs/latest), observed 0.1.1 | Documents vendored SDK, caller-owned D3D11 device, GPU APIs, Windows x64 MSVC | First binding to inspect; audit unsafe ownership, SDK provenance, device recovery and build reproducibility |
| [`spout-rs`](https://docs.rs/crate/spout-rs/latest), observed 0.2.0 | Documents D3D11 receive/shared-handle send and vendored option | Alternative; its documented wgpu staging-readback/upload example is not the required production path |
| [Microsoft windows-rs](https://github.com/microsoft/windows-rs) | Official Rust Windows API bindings | Preferred D3D/DXGI/process API boundary; keep raw types private |

Package availability is not a maintenance audit. The subsequent
[published-source/SDK audit](../progress/mapgyver-m01-native-feasibility.md#published-binding-audit)
and native ownership checks support the selected pinned sys binding; production supervision
and recovery still belong to M03/M04. The upstream
[Spout SDK license](https://github.com/leadedge/Spout2/blob/master/LICENSE) is BSD-2-Clause.
If no wrapper meets the gate, use a narrow owned binding to the upstream SDK, not a new protocol.

The native criterion is no recurring CPU readback/upload, not "zero copy." GPU copies/conversion
and initial picture decode/upload are legitimate. Qualify a single adapter first. On device
removal/reset recreate the device-dependent resources and reacquire endpoints, fencing stale
completions. [DXGI errors](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/dxgi-error).

## Owned application processes

Windows Job Objects group owned process trees and support kill-on-close. Create the child
suspended, assign it, then resume. Keep the job handle until the Matrix's final demand/unload
lease ends; support a graceful stop deadline before termination. Existing external instances,
elevation and launcher/singleton handoffs need explicit outcomes rather than name-based kills.
[Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects),
[job limits](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information).

## Native feasibility gate

The user-approved 2026-10-28 scope correction separates the M01 functional route decision from
the continuing native qualification matrix. M01 requires scoped native underlay/input evidence,
two-source D3D11/Spout composition without recurring CPU-frame transport, interruption/resize/
cleanup smoke, and a recorded host/backend/binding decision with later owners. These are the
[four fixed M01 acceptance items](../plan/MapGyver_Implementation_Plan.md#m01--decide-the-functional-native-rendering-route).
Their completion permits M02's shell and public host seam; it does not assert the original full
gate passed. Ordinary Tauri/Wry, D3D11 and `spout2-sys =0.1.1` are the selected route.

The full matrix below remains required at its implementation owners:

| Continuing requirement | Owner |
| --- | --- |
| Renderer CPU/GPU/cadence/frame-age budgets, bounded queues/resources, retirement and device recovery | M03; source supervision in M04 |
| Physical output/preview accuracy, DOM/input behavior, complete DPI/negative-origin matrix and measured alignment/latency | M05 |
| Integrated sustained workloads, complete-app recovery/shutdown and full product Q05 | M09, after each feature owner verifies its own behavior |

Use an ordinary build/probe process and observed native interaction. The user authorized desktop
automation on 2026-10-27; use the computer-use skill for native input and screenshots, with manual
interaction where necessary. Record unavailable
native checks honestly; browser screenshots do not qualify native texture layering.
NVIDIA qualification uses the approved [overlay-off precondition](../progress/mapgyver-qualification-contract.md#nvidia-overlay-prerequisite).
Overlay-on resource failures remain recorded; this condition does not replace the native gate.

- Run a packaged x64 probe with the locked host versions. Display two independently sized native
  preview regions with DOM handles, opaque panels, menus and dialogs composited correctly above.
- Exercise crop/pin pointer capture, outside-region dragging/release, resize, maximize/restore,
  docking, tab switches, scrolling and zoom. Verify cleanup hides stale/unmounted views and
  prevents persistent white/black flashes or exposed content behind opaque UI.
- Check 100%, 125%, 150% and 200% display scaling, mixed-DPI monitor moves and negative desktop
  coordinates. Record alignment error and layout-to-native update latency on reference hardware.
- Receive two concurrent Spout senders at different sizes; composite and present to a native
  output and a Spout sender without recurring CPU readback. Resize/restart either sender while
  preserving the other channel and consumer references.
- Verify output continues while the editor is minimized/busy/disconnected. Exercise output
  removal/recreation, missing sources, GPU reset where testable, shutdown and reopen.
- Verify alpha/color cards, report actual copies/conversions, and measure GPU/CPU frame time,
  latency, queue depth, memory/handles and sustained 1080p/4K behavior. Fix workload-specific
  budgets before qualification; do not infer throughput from one visible frame.

If ordinary hosting fails its implemented M05 presentation requirements, revisit the explicit
DirectComposition alternative before claiming editor quality. It does not justify CPU-frame
transport as the native preview solution. Missing/failed checks remain recorded at their owners;
M01's route decision neither erases them nor requires further diagnostic experiments before M02.
