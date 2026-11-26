# Repository split

Use [the split ledger](../progress/repository-split-status.md) to resume this migration. The product plans retain their own acceptance and qualification work.

## Ownership

| Repository | Source ownership |
| --- | --- |
| `Golden-Geek/GoldenCore` | `crates/`, `packages/`, `vendor/`, shared architecture, retained Golden Core and dependency notices, shared protocol generators, and reusable runtime, UI and engine checks. |
| `Golden-Geek/MapGyver2` | `apps/mapgyver/`, `tools/mapgyver/`, MapGyver notes, docs and evidence, native and installer qualification, and MapGyver release automation. |
| `Golden-Geek/Chataigne2` | `apps/chataigne/` and its systems, Chataigne notes, docs and evidence, app fixtures, desktop product qualification, and Chataigne packaging. |

Keep generic engine, audio and UI checks with Golden Core. Keep product gates and release configuration with their app. Give each checkout its own manifests, lockfiles, toolchain setup, and CI. Split the mixed development launcher into one launcher per app, keeping the existing commands that still apply. Move live design documentation with the layer it describes and use cross-repository links for shared references. Preserve links into historical source and evidence by using commit or run URLs.

## Milestones

1. **Record the source.** Verify the live Chataigne `main` and `mapgyver` tips and the MapGyver destination, record their immutable SHAs, and use an isolated migration branch. Use the reconciled `mapgyver` source for the extraction because it contains newer Chataigne changes too. Preserve the old branch and historical release refs.
2. **Extract Golden Core.** Import path-filtered history in an unpublished clone, preserving notices and attribution. Build a standalone Cargo/npm workspace and move generation and drift checks for shared TypeScript contracts to it. Give reusable browser and native fixtures independent entry points. Verify shared Rust and UI tests, lint, formatting, and generated contracts from a clean clone with no apps. Publish the verified commit before the apps pin it.
3. **Import MapGyver2.** Preserve the destination's initial license commit as an ancestor. Import product-only history from the same source and pin Golden Core at `golden/`. Fix native probe, preview, FFmpeg/NDI, vendored Cargo patch, installer, and CI paths. Verify a recursive clean clone, existing project reopen, desktop/headless startup, media preview and Screen output, and the existing desktop and ARM installer matrix through a non-publishing run. Route future MapGyver releases to MapGyver2 and keep historical downloads reachable at their original URLs.
4. **Cut over Chataigne2.** Use the migration branch to retain the newer Chataigne changes and original Git ancestry. Replace in-tree Golden with the verified `golden/` pin and remove the extracted MapGyver tree from the current branch. Verify a recursive clean clone, existing project save/reopen, desktop/headless startup, and Alchemist and state-machine editor smoke tests. Merge the cutover after both standalone apps pass.
5. **Verify independent upgrades.** Check that a Golden pin bump triggers the consuming app's CI, changes both the Rust and UI sources used by that build, and invalidates the frontend bundle. Verify the other app at its previous pin and leave accepted production pins on verified commits.

Each app uses Cargo paths and npm workspaces under `golden/`, with only the required Golden UI packages and their transitive workspace dependencies. Each application owns its Rust-to-TypeScript generators and consumes generated shared contracts from the Golden checkout. The application Cargo workspace owns any root patch declarations needed to activate Golden vendor patches. All build and test paths must work without a sibling checkout. Use clean status after builds to detect generated-file or lockfile drift.

Update the split ledger before each implementation batch. Commit and push coherent milestones after their checks. Advance only after verifying the pushes and record any open acceptance explicitly. Carry forward the products' existing hardware and performance obligations in their own ledgers. The migration uses verification workflows and does not issue a product release.
