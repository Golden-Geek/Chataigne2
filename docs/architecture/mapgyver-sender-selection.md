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

This checkpoint verifies the typed model, mode transitions, history, project
roundtrip, command execution and four public caller paths. M11/5 remains open:
the host-owned live catalog projection, inline Manual Detect adornment, and native
appearance/disappearance/restart/rename and recovery checks are still required.
The current Auto selector preserves its exact value but does not yet populate
new live sender options. The existing Manual discovery picker remains available.
