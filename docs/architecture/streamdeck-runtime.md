# Stream Deck service ownership

Stream Deck is app-owned under `apps/chataigne/src/module/modules/controllers/streamdeck`.
Golden IO provides worker admission, retained payloads, notifications and retirement; it owns
no Stream Deck policy. Local qualification and its limits belong to C11 in the
[implementation ledger](../progress/implementation-status.md).

`runtime/native.rs` uses the existing `elgato-streamdeck` crate. Discovery, connection,
zero-timeout HID reads, brightness, image loading/decoding, rendering and cleanup belong to
one native owner. Four process slots and one per-module slot include blocked setup and cleanup.
Worker termination invalidates native publications and wakes the actor before driver destruction.
Native endpoint construction is disabled in tests, including hardware-feature tests.
Read/render failure ends the native lifetime: invalidation and notification precede cleanup,
then passive service retries admission with 250 ms to 5 s backoff. A blocked cleanup keeps
both lifetime slots. Device selection changes reuse the current owner; catalog discovery uses
a coarse two-second deadline. Missing selected serials keep their identity in typed choice edits.

`output.rs` captures the active appearance and input routes behind retained admission. Visual
feedback coalesces to the latest capture; ordered key transitions retain their original route
and source lease. Mailbox, callback and choice-domain pressure pause new native input while
bounded feedback turns continue. Worker output renders at most four keys per turn. The actor
is passive; it projects prepared input/status through native and authored-source guards.
The mailbox admits 32 rows / 256 KiB and actor projection drains sixteen rows per wake. The
publication pool admits 128 lifetimes / 32 MiB, charging 8 KiB per row and 256 KiB per canonical
status. Choice domains retain their original shared allocation behind 128 lifetimes / 64 MiB.
Feedback captures retain 512 KiB each behind 64 lifetimes / 32 MiB, including captures still
held by accepted input. Native reports admit at most 64 edges; pending pressure never starts
another input read. A report that returns from a blocked call captures the then-current page.

`layout.rs` owns page identity allocation, default/derived controls and input mirrors. It scans
bounded metadata slices, prepares one page per turn and waits for edit disposition notifications.
Known missing subtrees use detached `NodeTree` edits. Script page requests capture a bounded
immutable template at invocation. `input.rs` admits editor/script presses directly as backend
intents, independently of hardware support or native owner capacity. Page selection and the
default-only Unpaged appearance flag retain their existing meaning.
Authoring and layout batches retain 1 MiB each behind 64 lifetimes / 64 MiB. The same request
lease covers its materialization, avoiding a second reservation when the authoring queue is
full. Template preflight covers materialized node storage, strings, values and all constraints
before cloning. Edit completion or rejected-suffix disposal wakes the next bounded layout turn;
idle receipt polling and idle native snapshot rebuilding are unnecessary.

Local bounds are 512 derived pages, 64 input slots, 32 configured visuals, 256 template nodes,
16 template levels and 256 KiB per template. Visual strings admit 1024 caption bytes and 4096
image-path bytes. Native image loading admits a regular file up to 8 MiB, image dimensions up to
2048 by 2048 and 16 MiB decoder allocation; rendered key images are at most 128 by 128. Native
caption rendering still needs a bundled font, as before. Vendor-private allocations, physical
devices and product-scale performance need their later qualification owners.
