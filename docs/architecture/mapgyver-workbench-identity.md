# MapGyver workbench identity

MapGyver owns its branding, media-kind icons and editor titles. Golden UI provides optional
app-header and panel icons, a live node-icon resolver, and the existing footer contribution.
No MapGyver types or assets live in Golden packages.

`mapgyver_icon.png` is the supplied source. Run `node tools/mapgyver/generate-icons.mjs`
from the repository root to regenerate desktop ICO/ICNS/PNG, favicon, header and pattern assets
using the installed Tauri CLI. Intermediate platform variants remain under `target/m119-icons`.
The Tauri bundle declares these app-owned desktop icons and explicitly sets the NSIS installer
and uninstaller icons. Signing and release delivery remain
separate from local package validation.

`apps/mapgyver/ui/src/lib/icons/tree.ts` registers supplied SVGs. The resolver reads the
immutable typed `parameters/media_kind` leaf through the public session, stopping at the
declared Parameters folder. It does not scan the graph or reinterpret container admission.
Static panel icons use the same assets where corresponding panels exist.

Matrix and Composition bounded backend projections carry their displayed object's label.
Surface editors use their existing Screen/Surface snapshots. `followPanelTitle` updates only
the owning panel's title; the existing controllers continue to own locks, selection and
independent panel state. Structural tab identity is `data-panel-type`, independent of titles.

Screen row projections resolve human-readable cached display names and report missing or
recovered targets without changing the authored backend identifier. Unusable names fall back
to a display number rather than exposing an internal path. Exact connected identity resolution
precedes output admission, so disabling an output cannot falsely mark its display missing.

The Video footer uses the public app contribution. CPU milliseconds measure the renderer API
call; FPS does not imply physical presentation. Memory is tracked allocation charge, formatted
as decimal MB/GB. The adapter label yields space at narrow widths. Golden's invisible idle
activity slot no longer reserves a permanent gap before the right-hand contribution.
Expired observation delivery clears stale metrics and shows `Video recovering` until real
host observations resume. The native fixture verifies expiry/recovery by suppressing delivery
through Tauri's debugging callback map; it does not invent host metrics.

Current acceptance and remaining checks belong to [the active ledger](../progress/mapgyver-status.md).
