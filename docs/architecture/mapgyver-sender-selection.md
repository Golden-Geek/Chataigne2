# Shared Texture sender selection

MapGyver owns the media-level `sender_selection` Enum (`Manual` or `Auto`). Each
Shared Texture channel owns a real `sender` String and a real `sender_choice` Enum.
The selected mode determines which Parameter projects into the runtime source.
Other media kinds do not materialize the mode or Auto selector.

Mode changes use Golden's atomic Parameter companion proposal. Each channel's
current exact sender is copied to the newly active leaf, unless that leaf is
explicitly included in the same proposal. One Undo restores the mode and all
affected leaves. Configuration intents, command nodes and named script methods
use these same Parameters. The default is Manual; no sender is selected implicitly.

Golden owns `ParameterEnumDomainPolicy::Open`: options describe availability,
while exact unlisted Enum selections remain valid authored values. Closed enums
retain their existing validation. The policy is declared through the Params DSL,
serialized by Rust and generated into the public UI protocol. The generic Enum
editor displays an unlisted current selection as unavailable. Availability updates
must change Enum metadata without changing authored values or history.

The media document is now `mapgyver.media.v8`. Earlier media schemas are rejected
by project admission; this work does not introduce a persisted-data migration.

The native desktop preparation worker and native headless host worker service a
coarse SDK catalog every 500 ms. The media manager receives one latest-only,
lifetime-fenced metadata update through Golden's `NodeWaker`. Each update checks
channel revision, Parameter UUID and expected value. It never writes a selected
value or adds history. Closed engines cannot accept updates. The available option
storage is shared across channels; unavailable selected tokens remain authored and
the generic Enum editor displays them without retaining an ever-growing name list.

Golden's `NodeStore::membership_revision` is a constant-time lifecycle identity,
including equal-size replacement and detach/reattach. The catalog reindexes only
after membership changes, and reads changed channel bindings after their existing
revision guards expire. Idle scans do not traverse or snapshot the document.
Catalog enumeration, option preparation and source reconnect run outside the
engine and UI threads.

Golden's public Parameter adornment registry renders app-owned contextual actions
alongside ordinary typed editors. Manual Detect reads the sibling Auto Enum's live
metadata and sends an ordinary String edit; Auto shows only the generic Enum.
Both controls receive updates through the same generic transport subscription in
desktop and headless clients. There is no separate UI discovery/reconnect loop.

Conditional sparse Parameter children restore their declared Enum domain policy
and missing static options when binding existing nodes after load. Existing live
catalog options remain intact. This preserves the editable Manual/Auto control
and prevents sender disappearance after project reload from rejecting its current
exact name.

M11/5 is Accepted. The [pinned acceptance receipt](../progress/evidence/mapgyver-m11-sender-selection-2026-11-04.json)
covers software, real headless controls and the native bundled WebView: open-list
appearance, inline Detect, Auto-only Enums, disappearance/restart/rename, independent
channels, foreign-adapter isolation, history, Save/New/Load, post-load mode changes
and post-load sender recovery. Eleven independent GPU images verify both outputs;
279/279 requests settle without unexpected rejection, and owned processes exit
normally. Screenshots are reviewed. Original timing/device-removal and complete
app-control retirement qualification remain separate gates.
