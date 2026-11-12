# Video policy for Matrix playback

MapGyver's Video owner declares **On Activate** and **On Deactivate** as real enum
Parameter children. They belong to the Video media, alongside its shared transport,
not to a Reference, channel decoder, panel or Golden's codec runtime.

The authoring slice provides settings and inspection. The runtime-use boundary
below applies those policies to explicit backend leases. Wiring actual Matrix
playback to these leases and native acceptance remain M10/7 work; changing a
setting alone leaves transport as it is.

The public Rust protocol defines `VideoOnActivate` (`PlayFromStart`, `Resume`,
`Nothing`) and `VideoOnDeactivate` (`Stop`, `Pause`, `Nothing`). Defaults are
`PlayFromStart` and `Stop`. Generic Inspector edits, typed media operations, command
nodes and named script calls (`setVideoOnActivate`, `setVideoOnDeactivate`) edit
the same Parameters. Their generated TypeScript declarations have no second
handwritten authority. Authored policies survive history, media copies and project
reload; channel copies keep their existing shared owner. Runtime position remains
transient and independent of these settings.

`VideoControl::activate_for_matrix` acquires a `MatrixVideoUse` against that shared
owner. Channels and References participating in the same committed activation
share its identity, so activation runs once. A fresh reactivation uses a fresh
identity. Admit incoming uses before releasing outgoing ones on a Cut. Calling
the factory for an already entirely outgoing identity retains an outgoing mirror;
it preserves the original manual-intent fence. Calling
`deactivate` begins outgoing retention; it captures the deactivation choice when
the last active consumer leaves. Keep the lease until the actual transition ends,
then `finish` it. Releasing the last outgoing use applies the captured policy only
if no newer activation or manual transport intent superseded it. Dropping an
active lease provides immediate withdrawal/cancellation. Live uses have counted
identities; completed identities leave no tombstone/history queue.

Manual Play/Pause/Stop, seeks and changed rate/loop/load intents fence a deferred
automatic action. Editing a policy alone leaves that fence intact and affects
future activations/deactivations. Retirement changes the use generation and clears
logical bookkeeping, so an old lease cannot act on a restored owner. Decoded/GPU
frames retain their existing completion owners; a logical clock action fabricates
no frame completion. Software policy tests use a zero playback rate for exact
positions; moving native images still require the full Matrix integration checks.
Ordinary Play rewinds at forward EOF and reverse zero. Matrix's literal zero-start
and retained-position policies still need endpoint/reverse reconciliation in the
next committed Cut slice before native acceptance.

The remaining Matrix integration must apply activation at committed playback
boundaries and defer **all** deactivation choices until the outgoing transition
ends. `Pause` then retains position; `Stop` resets it; `Nothing` leaves transport
unchanged. Outgoing content must keep moving until that boundary. Resolve shared
Video owners once across channels and References, retain other active consumers,
and fence stale actions on reactivation, interruption, deletion and project changes.
An expired transition must never stop a newly active or still-shared clock. This
is app-owned demand and transport policy; GPU completion and decoder ownership
remain at their existing public Golden boundaries.

The [implementation plan](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)
owns acceptance: all policy combinations, shared consumers and manual transport,
Matrix enabled/Stop behavior, all callers/history/copy/reload, and actual native
playback after software prerequisites. Authoring tests alone do not accept M10/7.
