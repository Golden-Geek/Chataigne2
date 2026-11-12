# Video policy for Matrix playback

MapGyver's Video owner declares **On Activate** and **On Deactivate** as real enum
Parameter children. They belong to the Video media, alongside its shared transport,
not to a Reference, channel decoder, panel or Golden's codec runtime.

The first implementation slice provides authoring and inspection only. Automatic
Matrix application and its acceptance remain M10/7 work; changing either setting
does not itself seek, play, pause or stop a Video.

The public Rust protocol defines `VideoOnActivate` (`PlayFromStart`, `Resume`,
`Nothing`) and `VideoOnDeactivate` (`Stop`, `Pause`, `Nothing`). Defaults are
`PlayFromStart` and `Stop`. Generic Inspector edits, typed media operations, command
nodes and named script calls (`setVideoOnActivate`, `setVideoOnDeactivate`) edit
the same Parameters. Their generated TypeScript declarations have no second
handwritten authority. Authored policies survive history, media copies and project
reload; channel copies keep their existing shared owner. Runtime position remains
transient and independent of these settings.

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
