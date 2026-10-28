# MapGyver native rendering research

Research date: 2026-10-27. These are source-backed feasibility findings and proposed gates,
not evidence that MapGyver has rendered a native frame. See [ownership and product contracts](mapgyver.md)
and the [implementation plan](../plan/MapGyver_Implementation_Plan.md).

## Transparent WebView over native rendering

The desired "hole" is a transparent area in the HTML/WebView revealing a native GPU-rendered
surface underneath. Opaque workbench chrome and DOM handles remain above it. Wry's official
[GPU example](https://github.com/tauri-apps/wry/blob/dev/examples/wgpu.rs) demonstrates native
rendering behind a transparent child WebView. On Windows it disables child clipping and enables
WebView transparency. The example is also present in the locally installed, locked Wry 0.55.1.
The checkout locks Tauri 2.11.5 and tauri-runtime-wry 2.11.4. The upstream moving example is
supporting evidence; the packaged probe must run against the repository's actual dependency lock.

Microsoft documents that transparent web content and a transparent default background reveal
the host's content. The default background accepts opaque or fully transparent alpha, not an
arbitrary intermediate value. [WebView2 background API](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller2)

Coollab is useful visual-product inspiration, but its current source selects OpenGL and exposes
native ImGui windows/menus. It is not evidence for this WebView implementation.
[Coollab build](https://github.com/Coollab-Art/Coollab/blob/main/CMakeLists.txt),
[Coollab app interface](https://github.com/Coollab-Art/Coollab/blob/main/src/App.h).

Golden's desktop host currently builds an opaque main WebView and owns its Tauri builder/setup.
Add an optional public presentation extension there rather than creating another desktop bootstrap
inside MapGyver. Tauri exposes public native window and platform-WebView access; its platform
API documentation advises pinning at least the minor version when using underlying types.
[Tauri window API](https://docs.rs/tauri/latest/x86_64-pc-windows-msvc/tauri/window/struct.Window.html),
[Tauri platform access](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/webview/mod.rs).

Recommended first probe: D3D11 rendering below the existing ordinary Tauri/Wry WebView, following
the layering pattern in Wry's wgpu example. The example proves the pattern, not this exact
D3D11/Tauri integration. Use one underlay per editor window and scissored preview regions.
Svelte owns pointer capture/hit testing; visual transparency does not imply native input routing.

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

Package availability is not a maintenance audit. Some deep wrapper source links were unavailable
during research; neither wrapper is selected yet. Review the published source, upstream revision,
license notices and target build before choosing. The upstream
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

Failure of the ordinary-hosting gate changes the presentation implementation decision before
editor rollout. It does not justify CPU-frame transport as the native preview solution.
