# Native preview fixtures

The default entry mounts `Fixture.svelte`, the finite registration/clipping smoke.
`?mode=qualification` mounts `Qualification.svelte`, which holds two registrations until the
host explicitly changes them. Neither fixture transports pixels or measures video cadence.

The held fixture emits `golden-qualification-fixture` with phase `listening` after installing
its metadata listeners. The host can then supply `golden-qualification-layout` and lifecycle
commands through `golden-qualification-command`. Rust owns these types and event constants in
`golden_video::qualification::protocol`; `generated/` contains their generated TypeScript.

Layout dimensions are CSS pixels computed by the host using authenticated native metrics.
Their inline CSS sizing deliberately implements this measured fixture contract. The UI never
uses `devicePixelRatio` to calculate them. Reports include measured CSS rectangles, installation
state and retirement masks; the host must validate actual physical sizes and clipping separately.
No default physical-size claim is made. Optional initial query inputs are `source1`, `source2`,
`width1`, `height1`, `width2`, `height2`; all six must be supplied together as CSS metadata.

Layout revisions and command IDs increase monotonically. Layout updates coalesce to the latest
pair, and at most one command waits behind an active operation. `ready` renegotiates the preview
session before remeasuring, allowing recovery after a minimized document's lease expires. `hide` and
`show` preserve the two DOM registrations, and `disconnect` waits for an acknowledged clear
before disconnecting the session. None of these actions runs on a timer. Both views retain
independent DOM/SVG overlays, including working buttons after disconnection.

Run `npm run test:browser --workspace golden_video_ui` for the actual bundled Svelte fixtures
with a metadata-only fake native adapter. This verifies browser layout, input and lifecycle
ordering; native pixels, physical dimensions and GPU timing require the owned native runner.
