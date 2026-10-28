# MapGyver M00 baseline

MapGyver owns its product systems; Golden owns the shared host, control, persistence and GPU
mechanisms. This handoff reconciles their existing boundaries before the native feasibility work.
The [execution ledger](mapgyver-status.md) remains authoritative. Inventory is not a native pass.

## Checkout and preservation

Implementation was explicitly requested on 2026-10-27. Starting HEAD was
`137c9203dc2afa21a5c55eb1b8fadb32fad473b0` on `main`. The user then requested the new
`mapgyver` branch, created at that HEAD with all working-tree edits preserved. During M00,
an external fast-forward from `main` advanced it to `40acd67df94920b63469a77da0ee64e40368d60b`
(the Chataigne P11 checkpoint). This agent performed no merge, commit or publication.

The [captured inventory](evidence/mapgyver-baseline-2026-10-27.json) records the checkout before
the branch switch, dependency hashes, hardware and tool versions. Its `main` branch field is
historical. Existing dirty files include the incremental process-snapshot candidate, engine
revision/read-view changes, Spatializer tests, P11 evidence and architecture/planning docs.
Those implementation files were subsequently checkpointed by `40acd67d`; the remaining dirty
root/architecture docs retain their original bytes. M00 neither validates nor absorbs the
Chataigne implementation.

The [Chataigne ledger](implementation-status.md) reports P01-P10 locally complete, P11 at four
of six checkpoints, and P12-P18 unstarted. The 100k Spatializer creation completion and final
counter-refresh timing, normal-build and native preservation checks remain open. Existing commits, archived task lists and the IDE's
old `submodules/golden_core` tabs do not change the current `crates/golden_core` ownership.
Cargo default members and root npm commands still select Chataigne.

## Public contracts to consume

| Owner | Existing public entry | MapGyver use / remaining gap |
| --- | --- | --- |
| Golden engine app | `ProjectNode`, `ProjectLifecycle`, `ProjectFileSpec`, `NodeTree` through `golden_core` | App node registration, identity/file policy, batched default trees and backend app operations; no Chataigne private imports |
| Golden desktop | `run_default_with_ui_assets_and_dev_server`, `FrontendDevServerConfig`, launch APIs | Shared desktop/headless startup; typed native-presentation extension still absent |
| Golden transport | `UiAsset`, `UiServerConfig`, `UiPreferencesConfig` | Bundled assets, transport and preferences; discovery still hardcodes `chataigne` and `/.well-known/chataigne` |
| Golden production runtime | `ProductionRuntime`, `ProjectGeneration`, control/compilation contracts | Atomic authored changes, project replacement and stale-result fencing; GPU resources stay outside the engine |
| Golden persistence | Project document codec and save coordinator | Reuse authored persistence/recovery; native resources are never project data |
| Golden IO | Bounded workers, payloads, retirement, revision guards, `ReconnectBackoff` | Worker admission/disposal and endpoint recovery; no public general owned-process API yet |
| Golden codegen | App-node generation, protocol/graph TS generation, `build_cache` | Rust-owned DTO generation; reusable UI bundle preparation still lives in Chataigne's build script |
| Golden UI | Workbench/panel registrations, transport interface and focused stores | Reuse the workbench; branding, folder labels and browser storage still need app identity |
| Golden Audio | Existing render clock, decode/cache workers | Preserve audio semantics; these do not establish project-wide production time or persistent asset identity |

Start from [application contracts](../architecture/application-contracts.md), then the public
crate exports. Specifically, M02 repairs `packages/golden-ui/components/app/AppHeader.svelte`,
warning titles, `store/project-files.svelte.ts`, `store/ui-persistence.ts` and the Golden
transport discovery contract at their owners. It extracts the exercised UI build operation
from `apps/chataigne/build.rs`; formula generation and native module SDK steps remain app-owned.
Changing protocol DTOs requires generated output and consumers in the same batch.

These repairs cover only exercised P12 boundaries. P12 as a whole remains open, including its
incremental MSVC artifact gap. No MapGyver-specific branch belongs in Golden code.

P14 remains open: `IoClock` and the audio render clock are existing narrower contracts, not an
integrated show/monotonic/discontinuity authority. M03 must implement the required reusable time
boundary and fake-clock consumer evidence at Golden owners. P15 likewise remains open: audio
source/cache identity does not supply persistent project asset IDs, locators, relocation/relink,
or cancellation/generation contracts for image preparation. M03 adds those pieces once and
records shared evidence without declaring all Chataigne audio acceptance complete.

## Compatibility inventory

The initial engineering qualification floor is **Windows 11 24H2, build 26100, x64 MSVC**.
This is a chosen test target, not a demonstrated minimum or an installer restriction. M09 must
exercise that floor; the current machine alone cannot qualify it. Earlier Windows versions,
ARM, other OSes and cross-adapter interoperability have no acceptance claim.

| Component | Observed on 2026-10-27 |
| --- | --- |
| OS | Windows 11 Famille 26H2, `10.0.26300.9711`, 64-bit |
| CPU / RAM | Intel Core Ultra 9 275HX, 24 logical processors; 33,752,997,888 bytes physical RAM |
| Intel GPU | Intel Graphics, driver `32.0.101.8331`, driver date 2026-01-29; CIM reports 2560×1600 at 240 Hz |
| NVIDIA GPU | RTX 5080 Laptop GPU, driver `32.0.15.9201`, driver date 2026-02-15; CIM reports 3840×2160 at 60 Hz |
| Monitors | Two active WMI connection records; physical origins, DisplayConfig IDs, DPI and adapter routing unmeasured |
| WebView2 Runtime | `155.0.4283.62` from installed runtime registration |
| Rust | rustc/Cargo `1.97.0`; `x86_64-pc-windows-msvc` directory override |
| JS | Node `26.5.0`, npm `11.17.0` |
| Native tools | CMake `4.4.1`; VS Build Tools `18.5.11723.231`, VS Community `18.10.12224.181` with x64 VC tools registered |
| Locked host | Tauri `2.11.5`, tauri-runtime-wry `2.11.4`, Wry `0.55.1`, webview2-com `0.38.2`, raw-window-handle `0.6.2` |
| Windows bindings in lock | `windows` `0.48.0`, `0.61.3`, `0.62.2`; avoid accidental COM-type mixing at a new boundary |

CIM resolutions are inventory hints, not monitor identity or successful presentation. Do not use
its 32-bit `AdapterRAM` field as a VRAM budget; M01 must obtain DXGI adapter LUIDs and memory
budgets. Registered VC tools do not prove the chosen C++ SDK compiles. Record the actually
selected compiler/SDK when the native probe builds.

## Native routes and binding candidates

| Route | Current availability and evidence required next |
| --- | --- |
| Locked ordinary Tauri/Wry underlay | Source available, not executed. Installed Wry's `examples/wgpu.rs` uses transparent child WebView and disables child clipping; it is not D3D11/Tauri proof |
| D3D11 / native outputs | Two GPU/controller records and a 4K60 mode available as test candidates; actual device creation, routing, presentation and feature level untested |
| Spout GPU sharing | No SDK dependency in workspace and no process name matching Spout/MapGyver at capture; SDK discovery is untested, so usable external senders are unknown |
| Mixed DPI / negative origins | Two active monitors offer a candidate route; current scales/origins unmeasured. Manual qualification remains required; no display settings changed |
| Device loss / floor OS | Unavailable in M00; later injected/native recovery runs and a separate floor-OS machine/image needed |
| DirectComposition fallback | Not implemented or selected. Investigate only if the ordinary-host gate fails; no composition-controller cast workaround |

First audit candidate is [`spout2-rs` 0.1.1](https://docs.rs/crate/spout2-rs/latest), documenting
a vendored Spout 2.007.017 SDK and caller-device D3D11 APIs. Alternative is
[`spout-rs` 0.2.0](https://docs.rs/spout-rs/latest/spout_rs/). These are candidate versions,
not workspace selections. Upstream [Spout2's license](https://github.com/leadedge/Spout2/blob/master/LICENSE)
is BSD-2-Clause. M01 must inspect published wrapper/sys sources, exact vendored SDK provenance,
license files, pointer ownership/thread rules, release paths and resize/device recovery before
choosing a dependency. A README or successful documentation build cannot close that gate.

The fixed [qualification contract](mapgyver-qualification-contract.md) defines scenes and budgets
before any native run. Reproduce inventory with:

```powershell
powershell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File tools/mapgyver/collect-baseline.ps1
```

The collector emits JSON to stdout, reports unavailable queries explicitly, and never launches
windows, changes displays or synthesizes input. It intentionally omits machine names, monitor
serial numbers and arbitrary process command lines. Native visual/input gates require manual
interaction with the future probe; unavailable checks remain open.
