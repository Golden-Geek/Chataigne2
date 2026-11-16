# Screen creation defaults

MapGyver owns fresh Screen allocation in `screens/creation.rs`. Golden's desktop host supplies
physical display metadata, including an optional internal-connection classification independent
of the primary-display flag. Unknown/virtual connections are not automatic destinations.

Fresh creation chooses the unused known external display with the smallest stable device-path
identity. Disabled fullscreen assignments still reserve their matched physical destination.
No available external means the typed Target remains Fullscreen and Display remains empty;
the render projection is unconfigured and creates no output. Explicit Shared Texture and
Rectangle choices remain ordinary authored settings.

`screens/host_inventory.rs` maps the public host metadata once on a dedicated worker before
seeding each detached startup/New candidate. It seeds both the initial Screen and the ephemeral
manager catalog without history. A failed scan is reported and leaves Fullscreen unselected.
Live inventory refresh reuses that mapping on the existing preparation worker. No IO is added
to live creation transactions or recurring engine callbacks.

Golden's read-only `UserItemCreationContext` lazily visits earlier prepared children in an
atomic creation batch. MapGyver reserves their assignments alongside committed siblings, so
detached multi-create cannot claim one display twice. Factories that do not use this reader
incur no prepared-tree scan or reference-array allocation. The context never publishes or
mutates detached siblings.

Intrinsic constructors and sparse decoding defaults remain stable. Loaded, duplicated and
reconnected authored targets are not reallocated by this fresh-creation policy. Existing
runtime display matching and disappearance/recovery remain responsible for output routing.
Acceptance, including missing physical topologies, is recorded in the MapGyver status ledger.
