# Golden Assets

Golden Assets separates persistent source references from disposable preparation. Apps own their
catalogs, import/collect/relink/history policy and project envelopes. Codecs own media formats;
Video/Audio own execution resources. This crate depends only on Golden IO and data/hash helpers,
with no engine, app, Tauri, image decoder or native resource dependency.

- `AssetId` is a persistent UUID. Relocation/relink changes the locator, never the ID. It is
  distinct from content fingerprints, cache keys, prepared leases and GPU/device generations.
- `AssetRecord` contains only that ID and an `AssetLocator`. Relative locators use validated
  forward-slash segments; absolute locators use the target's native rules. Both retain at most
  32,768 UTF-8 bytes. Serde validates paths on decode; the same Rust types derive TypeScript.
  Lexical resolution is not a filesystem sandbox. Sources reached through links remain ordinary
  filesystem sources. Absolute paths are machine-specific; relative paths follow project moves.
- `AssetScope` owns a runtime project directory/lifetime; `AssetBinding` owns one runtime record
  revision. Capture, relocation, relink and invalidation perform no IO. A replacement project
  needs a new scope; removed bindings must be invalidated or dropped even if history retains
  their authored record. None of these clocks is serialized.
- `AssetPreparer<D>` runs a trusted backend `AssetDecoder` on one bounded Golden IO worker.
  It resolves/opens sources, checks regular-file/encoded-size limits, stages and hashes immutable
  encoded bytes, decodes those bytes, then reopens and hashes the resolved path before publishing.
  Reopening detects atomic path
  replacement that inspecting the old handle would miss. Fingerprints are before/after content
  observations, not immutable filesystem snapshots; external changes may follow admission.
- `PreparedAsset<T>` is a cloneable Golden IO payload lease. Check `is_current()` at downstream
  admission and forward `guards()` across later preparation/installation. Keep the lease while
  its data is needed. No decoded bytes or backend handles belong in persistence/control JSON.

Command and outcome queues have separate item/weight bounds. Preparation reserves maximum encoded
staging plus decode allocation and captured metadata/guard/lease overhead before source staging
or decoder entry. After encoded storage is dropped, that reservation shrinks to decode and
metadata storage. The remaining conservative charge stays through all consumers and actual final
destruction; it is not freed when an outcome is drained or a preparer disappears. There is no
cache or automatic deduplication: duplicate references keep the same authored ID/fingerprint,
while a consumer may share one prepared lease. Rejected work preserves existing prepared leases.

Decoders must enforce their allocation budget during decoding, including output and scratch;
reported retained size is checked again before acceptance. The quota is a declared CPU-resource
bound, not an allocator/OS-memory measurement. Fixed worker/hash state, bounded transient paths,
thread stacks and filesystem handles remain separate overhead. Files
are bounded by encoded size; decoder reads additionally have an aggregate `4 × max_source_bytes`
budget and seeks stay within the admitted file length. Reads/seeks/hash chunks check cancellation
and scope/binding guards. Decoder reads use the immutable staged bytes, so temporary in-place
writes cannot produce a value inconsistent with its fingerprint. A blocking OS/codec call keeps
the sole lifetime slot until it returns.

Draining is nonblocking. While active, outcome backpressure waits on the worker rather than
dropping a result. Explicit cancellation/drop ends the lifetime and may discard queued work or
outcomes; it never joins or certifies retirement. `new_notified` wakes an existing owner after
outcome publication/disconnection. Release outcomes and consumer leases before a host-shutdown
worker waits on `AssetRetirement`. Final prepared-value destruction always runs on the payload
disposer. Consumers own change notifications, retry/recovery policy and format-specific caches.

Run `cargo test --locked -p golden_assets`. The public Video synthetic-descriptor consumer is
`crates/golden_video/tests/asset_preparation.rs`: it installs a valid null generation, then rejects
an invalidated prepared descriptor before scheduled installation while preserving the live plan.
MapGyver now also consumes this boundary for app-owned catalog/history, Picture decoding and
immutable Video image upload. `AssetScope::capture_record` lets an authored node forward its
existing lifetime guard, and `PreparedAsset::retained_bytes` exposes the conservative sealed
charge to downstream admission. Native Video inputs automatically forward image guards. These
consumers do not complete Chataigne's audio migration/collect/scheduled-playback P15 acceptance
or full GPU qualification. See the
[app integration evidence](../../docs/progress/mapgyver-m03-render-foundations.md#app-assets-and-picture-resources--2026-10-29).
