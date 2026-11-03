# M04/1 source authoring acceptance

MapGyver owns source definitions, typed command nodes, intent projection and embedded script
snippets under `apps/mapgyver/src/media`. Golden owns typed Parameters, generic Inspector
widgets, script execution, bounded operation delivery, history and persistence. No source
defaults or graph mutation policy live in the UI. See the
[source automation contract](../architecture/mapgyver-source-api.md).

**Accepted: 2026-11-02.** This closes revised M04/1: default Media Manager, detached
creation/import, authoritative identity/defaults and complete command/function/callback/snippet
API. Native source rendering and full revised project/resource parity remain separate gates.

| Requirement                                         | Authoritative implementation and exercised proof                                                                                                                                                                                                                                                                                                                                                       |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Default manager and backend-owned creation/defaults | `media/authoring.rs` initializes the manager and builds detached media/channel trees. `tests/creation.rs::manager_catalog_and_api_share_defaults_unique_identity_and_atomic_history` compares generic creation and direct intents for all five media kinds, unique identities, derived-output rejection and reload. The ordinary headless host starts with an empty manager.                           |
| Atomic Picture import and identity                  | `media/importing.rs` validates before queuing detached asset/media trees or assigning an existing variant. `tests/picture_import.rs` verifies one history step, no orphan assets on rejection, returned committed identities, preserved shared settings and QuickJS callbacks.                                                                                                                         |
| Revised media/variant ownership                     | `media/media_node.rs` owns Resolution, Sampling and procedural/Matrix settings; `channel_node.rs` owns variant source settings. `tests/parameters.rs` verifies generic/direct edits, integer bounds, stable Parameter identities, all-variant projection and roundtrip. `tests/channels.rs` verifies closed declared folders and source-only variant operations.                                       |
| Complete source commands and methods                | `media/commands` provides 28 real Parameter command kinds, Trigger/node-method execution and the catalog; `source_api.rs` exposes named methods, and `scripting.rs::manager_method` retains the full typed `editMedia` surface. `commands/tests` exercises every kind, passive creation/configuration, captured settings, disable/removal cancellation, copy, reload and atomic history.               |
| Same committed semantics for callers                | `tests/scripting/parity.rs` compares direct/UI intents, QuickJS app operations, hosted `editMedia` and named methods for source creation, settings, assets, ordering, inspection and rejection. Allocating results identify committed objects; reads/rejections add no history; Undo/Redo does not replay requests.                                                                                    |
| Applied/rejected callbacks and embedded snippets    | `scripting.rs` uses Golden's bounded queue and retained payload ownership. `tests/scripting.rs`, `tests/source_api.rs` and command listener tests verify committed results, rejection without partial edits, callback chaining and lifetime behavior. `script_templates.rs` embeds both app snippets; tests verify the registered method examples, passive loading and stable script identity/history. |
| Generated contracts                                 | `tests/typescript.rs::checked_in_typescript_matches_rust_contracts` compares fresh generation, including command kinds/catalog/field metadata, with all checked-in TypeScript and removes obsolete DTOs.                                                                                                                                                                                               |
| Usable generic Inspector and creation catalogs      | Bundled Resolution, procedural media, Channels, sender-only Shared Texture, command, script and catalog browser workflows exercise shared selection, actual generic controls, closed folder capabilities, variant/history operations and bounded 32-entry Channels rendering. Manager object menus contain neither channels nor automation nodes.                                                      |

Paths in the table are relative to `apps/mapgyver/src`; browser checks live under
`apps/mapgyver/tests/browser` and use the ordinary `tests/headless/media.mjs` host harness.

The combined browser check exposed a real shared-widget history defect: rapid vector-field
edits could leave an asynchronous edit session open, causing command Undo to restore its
configuration too. Golden's `MultiNumberEditor` now commits text fields as discrete parameter
writes, retains drafts until acknowledgement, and groups slider gestures only. The shared
edit-session helper cancels/finishes queued reopens and guards host/project lifetime. Eleven
focused ownership tests cover delayed acknowledgements, cancellation and replacement. The
command browser check now reads the exact backend Resolution Parameter through a depth-zero
query before/after Undo and verifies its final Inspector projection.

Verification:

- `cargo test --locked -p MapGyver --lib --quiet`: **285 passed, one existing ignored**.
- `npm run test --workspace golden_ui`: **228 passed** across 43 files.
- MapGyver and Chataigne Svelte checks: **zero errors and warnings**.
- Strict all-target/native-qualification MapGyver Clippy passed at `9fccf9ee`; Rust code and
  generated contracts are unchanged in this acceptance correction.
- Bundled host: `--channels-ui --resolution-ui --media-command-ui --media-script-ui --catalog-ui`
  passed, including canonical command configuration retention and source Save/New/Load.
  Windows locked the ordinary executable when a user opened `MapGyver.exe --dev` during the
  batch. The same `src/main.rs`, default features, app library and rebuilt bundled UI were
  compiled as temporary Cargo target `MapGyverAcceptance`; the manifest entries were removed.
  The harness's `MAPGYVER_TEST_BINARY` override selects that artifact without stopping the
  user's app. This verifies app behavior, not Windows release packaging.

No native experiment ran for this source API gate. Subsequent
[M04/2 acceptance](mapgyver-m04-variants.md) verifies native variants, shared Resolution and
channel identity on both hardware adapters. [M04/4 Spout acceptance](mapgyver-m04-spout.md)
subsequently passed on 2026-11-03, completing the formerly unfinished metadata/pixel/adapter cases.
M04/5 still requires its own complete interaction audit. M04/6 retains missing/relinked/replaced
Picture behavior, full revised caller/project/resource parity and the broader legacy
`--source-ui` harness port. Reference, Media Preview and all other plan gates remain required.
