# Matrix enabled metadata (M14/9)

MapGyver owns Matrix enablement in Layer, Group and Cell node metadata. The generic
Golden Inspector and manager enable switches edit that same state; Matrix configure
intents project `settings.enabled` to a metadata edit in the same transaction as the
ordinary typed Parameters. There is no separate Render Enabled or Enabled Parameter.

Disabled rows and sparse Cells remain in Matrix inspection and editable selections.
The render index and captured clip settings suppress disabled content without deleting
the launched clip, replacing its source or issuing Stop. The existing media routing
rule keeps Matrix visibility separate from media/application lifetime and now also
serves App Control availability and isolated content editing. An inactive disabled
Cell is skipped by launch/preparation; existing active/outgoing application holds
remain governed by their playback lifetime. A hidden Layer can still receive a backend
launch. Whole-Matrix, media, channel and ordinary folder disablement retain their
existing lifecycle behavior.

Layer v4, Group v3 and Cell v5 replace the schemas with duplicate enable Parameters.
Earlier versions reject before recoverable project loading, rather than silently
discarding an authored flag. Backend references, commands, UI manager filters and
context menus use the new types together. DTO shapes remain unchanged; generated
contract equality remains required.

## Verification

Implementation batch: 2026-11-17,09:20–10:00 UTC. Normal behavior checks cover direct
metadata and configure intents, active rendering, disabled destination launch,
application demand, video policy, editable inactive previews, history, subtree copy
and project reload. The existing independent native Cell fixture also checks current
Cell/Layer metadata off/on through two outputs and the production preview compositor.

UI static check: zero errors/warnings. Matrix/media-tree Node tests:41 passed. The
first UI test invocation incorrectly used Vitest for Node test sources; it provided
no suite result and was replaced by the passing native Node runner. The first Rust
compile found an omitted capture caller of the updated availability method. The
first complete software run found three normal-path regressions (inactive preparation,
hidden-Layer launch and editable disabled-Cell preview); these drove the corrections
above. The current full native-feature app run passes723 tests with8 ignored and
one old expectation that disabling an already held Cell stops its application.
That expectation is corrected to the specified render-suppression behavior; the
rebuilt owning application cohort passes5/5. This is not a new full-suite pass.
The real browser fixture then passes the complete Layer/Group and Cell Inspector
scope, per-field Undo/Redo, offscreen editing and mixed selection. The ordinary host
also passes source intents, Picture Files, catalog/rejection/history and Save/New/Load,
and exits normally. The first browser attempt stopped at one remaining obsolete
Enabled-checkbox locator; it is updated to the normal metadata switch.

Strict all-target Clippy with native-qualification/application-process-fixture passes
with warnings denied. Root and Golden Core formatting pass at checkpoint.

The09:20–10:00 batch is at cutoff. Checkpoint this feature;
do not open another recovery/profiling experiment. Native
Cell/Layer output verification and a fresh complete software cohort remain pending.

M14/9 is Pending qualification and unaccepted until its required checks pass. M14/10
fading Layer Stop is the next Active feature under the user's feature-first order. Recovery/stress
and performance work remain in the separately deferred qualification queue; this
feature does not close those gates or alter their budgets.

## Second-pass verification, 2026-11-20

- The bundled Inspector check passed for Layers, Groups and Cells, including offscreen editing,
  absence of duplicate enable controls, per-field Undo/Redo and mixed selection. The ordinary
  headless host exited normally. UI typechecking and 51 Matrix/media-tree tests also passed.
- After updating stale catalog, declaration and Reference-output expectations, the isolated
  software run passed 852 app tests and all 11 Matrix integration tests. Its one app failure
  exposed a reload defect. Schema-owned Audio and Subtitles folders gained open-folder edit
  permissions during hydration. The correction keeps their default permissions and preserves
  the ordinary editable-folder behavior. Strict Golden Engine Clippy passed.
- The app-wide 4,096-branch Composition case runs separately. A fresh ordinary host confirmed
  the updated subtree sizes of 15 nodes per leaf and 47 per branch with two References. The
  corrected large-case result remains unverified and is required for final app qualification.
- The permission fix passed 18 focused app tests, including exact Cell relocation and reload and
  metadata changes during an outgoing fade. The rebuilt Matrix fixture passed all 11 software
  cases. Both native tests passed on Intel and NVIDIA across 96 images and 73,728 texels,
  covering live visibility, disabled destination launches and editable inactive Cells. Physical
  outputs stayed closed.
- **M14/9 Accepted.** Receipts: `target/mapgyver-m14-9-inspector-155335.log`,
  `target/mapgyver-m14-9-isolated-software-161109.log` (reproduced failure), and
  `target/mapgyver-m14-9-pixels-162652.log`. The full app regression and large Composition case
  remain required before release.
