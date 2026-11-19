# Screen creation defaults

MapGyver owns fresh Screen allocation in `screens/creation.rs`. Golden's desktop host supplies
physical display metadata, including an optional internal-connection classification independent
of the primary-display flag. Unknown/virtual connections are not automatic destinations.

Fresh creation chooses the unused known external display with the lowest Windows display number.
Stable device paths resolve ties and order external displays whose number is unavailable.
Disabled fullscreen assignments still reserve their matched physical destination.
No available external means the typed Target remains Fullscreen and Display remains empty;
the render projection is unconfigured and creates no output. Explicit Shared Texture and
Rectangle choices remain ordinary authored settings.
Configuring an unselected Fullscreen Screen preserves that mode when changing frame rate,
AA or other settings; a null runtime destination does not imply a Shared Texture mode change.

`screens/host_inventory.rs` maps the public host metadata once on a dedicated worker before
seeding each detached startup/New candidate. It seeds both the initial Screen and the ephemeral
manager catalog without history. A failed scan is reported and leaves Fullscreen unselected.
Live inventory refresh reuses that mapping on the existing preparation worker. No IO is added
to live creation transactions or recurring engine callbacks.

Project-open preparation settles a fresh candidate's initialization forest, then prepares
metadata only if its manager has no catalog. Runtime inventory distinguishes unprepared from
prepared-empty state, so a disconnected desktop does not cause repeated preparation. Loaded
choices remain untouched. Headless hosting refreshes the same catalog once per second on its
existing application worker; unchanged scans do not enter the engine actor. Changed topology,
scan failure/recovery or an authored/lifetime fence publishes through the existing catalog
inbox. A retired publisher causes capture from the current project. No new thread, host polling
callback on the engine, or authored metadata store is introduced by the headless service.

Golden's read-only `UserItemCreationContext` lazily visits earlier prepared children in an
atomic creation batch. MapGyver reserves their assignments alongside committed siblings, so
detached multi-create cannot claim one display twice. Factories that do not use this reader
incur no prepared-tree scan or reference-array allocation. The context never publishes or
mutates detached siblings.

Intrinsic constructors and sparse decoding defaults remain stable. Loaded, duplicated and
reconnected authored targets are not reallocated by this fresh-creation policy. Existing
runtime display matching and disappearance/recovery remain responsible for output routing.
Acceptance, including missing physical topologies, is recorded in the MapGyver status ledger.

Debug GPU fixtures that use Spout or NDI can set `MAPGYVER_AUTOMATION_DISABLE_DISPLAY_OUTPUTS=1`.
The desktop host keeps the authored targets and display catalog available and admits only
non-window Screen output bindings for that process.
