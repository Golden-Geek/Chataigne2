# Reference output ownership

MapGyver owns Reference routing in `apps/mapgyver/src/media/reference`. A Reference v4
contains typed Media and Channel Parameters and exactly one declared, independently
addressable Output child. Its output UUID differs from the producer's channel UUID and
survives source changes, missing sources, history and project reload. Golden's
[sparse identity policy](sparse-node-identity.md) preserves this declared child's identity.

Changing Media selects its first exposed channel in the backend's atomic Parameter
proposal. An explicit Channel proposal wins; setting the same Media retains the selection.
Clearing Media clears Channel. An empty Channel follows the first exposed output, while an
explicit missing UUID remains authored for recovery and never silently reroutes. A resolved
channel belonging to another Media rejects. Reference chains and mixed Composition/Matrix
dependencies use the selected source for cycle validation.

The Output node contains no source settings, decoder or producer. Capture resolves its
Reference input and shares the existing source runtime. Composition placements and Matrix
Cells select the owned Reference's output; their selection no longer mirrors the producer's
channel list. Copies use the existing subtree identity remapping and preserve external
source bindings. Unconfigured References still expose their stable Output with separate
source availability status.

`SelectReferenceChannel`, its typed command and `local.selectReferenceChannel` edit the same
Channel Parameter. Catalog queries, source drops, preview selection, capture and project
validation consume that single model. Reference v3 projects are rejected explicitly; this
schema change does not provide backward compatibility.

The generic Inspector uses Golden UI's public `registerReferenceChoiceProvider` hook for
Surface and Reference targeting. MapGyver's `media::target_picker` supplies the protocol,
current-binding description, ordered catalog-folder/channel pages and validated selection
intent. Each request reads at most64 choices and256 sibling records; UUID cursors validate
against their owning scope, and folder ancestry admits only organizing folders under Medias.
Current bindings are queried independently of browse choices, so an explicit nested Cell
reference remains readable without exposing nested media as selectable catalog entries.

`SelectTargetPickerChoice` validates current catalog ancestry or exposed Channel membership
before writing the same real Parameter. Its ordinary backend companion proposal supplies the
default Channel and one history item. Explicit Cell operations retain their separate nested
source/cycle policy. Golden owns the reusable paged menu, loading/error presentation and read
lifetime fencing; no app types or defaults enter Golden. Metadata-only reads avoid loading
choices for closed controls. The [M13/14 acceptance audit](../progress/evidence/mapgyver-m13-targeting-acceptance-2026-11-08.json)
owns the complete caller/browser/history/project, large-catalog and both-GPU routing/sharing
proof. Other desktop, timing and production qualification remains separate.

Picker reads return an absent page when history or Cell replacement has removed the queried
Parameter. Reads may already be in flight before the Inspector receives that removal; this
ordinary absence must not produce a rejected mutation receipt. Selection still requires a
live, eligible Parameter and target. Rust generates the nullable result for every UI consumer.

Playback qualification follows Golden's normal load lifecycle: activate a decoded engine with
`prepare_engine_for_runtime` before launching or capturing it. A decoded candidate still has
bootstrap callbacks and inbox events; querying its authored definitions does not activate it.
The mixed Reference fixture uses this same production boundary.
