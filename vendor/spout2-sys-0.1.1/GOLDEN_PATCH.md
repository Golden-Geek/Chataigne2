# Golden's Spout binding patch

This directory owns a narrow extension to the public `spout2-sys` C/Rust binding and its
statically linked SDK. Golden Video consumes that public API; app crates do not include private
SDK headers or source files. The root Cargo patch selects this source for version `0.1.1`.
It is a local source replacement, not a new published dependency version.

## Source and license

The original package is `spout2-sys 0.1.1`, registry checksum
`670dd837cb0132982b6113a6c553e99453b0f8ba3014bd697bd7236e1a9d88aa`, wrapper VCS revision
`04db4225b261b8dacc8da0eeae9734bd75320557`. Its embedded SDK is Spout `2.007.017`, revision
`f49e2f469f8cb25f559a6eaa61a3f5b8173fc100`, independently audited in the
[native route evidence](../../docs/progress/mapgyver-m01-native-feasibility.md).

[UPSTREAM.json](UPSTREAM.json) records all 37 published files, their original byte lengths and
SHA-256 hashes, and the normalized baseline hashes. CRLF becomes LF; package VCS metadata gains
a terminal newline. One CP1252 author comment in `SpoutCopy.cpp` becomes UTF-8, retaining
Aurélien Vallée's credit. Registry cache metadata is excluded. The package's BSD-2-Clause
declaration and both supplied notices remain intact:
[root SDK license](vendor/Spout2/LICENSE), [SDK notice](vendor/Spout2/SPOUTSDK/licence.txt).

Upstream source layout, including its long implementation files and centralized FFI declarations,
is retained to make the fork auditable against the pinned release. Those third-party files are
excluded from workspace formatting, whitespace rewrites and source decomposition. Git retains
LF for stable normalized hashes; Golden-owned runtime and tests
continue to follow repository layout and size rules. Do not perform unrelated upstream cleanup.

## Extension ownership

The local patch has five responsibilities:

- Report the exact result of each DX11 send/receive attempt. A retained legacy new-frame flag
  cannot establish that the current call copied an image. `Copied` follows actual copy/flush;
  handshake updates, unchanged contents, busy access and disconnection remain distinct.
- Enable sender counting per instance and open receiver counters only for an explicitly known
  versioned peer. The patch never calls the registry-writing `SetFrameCount` API. An unknown
  external sender uses the unversioned path even if an old positive semaphore still exists.
- Offer a synchronized, nonblocking CPU counter hint so proven unchanged polls can skip GPU
  query/copy/flush batches. A missing, busy or invalid observation is not proof of no new frame.
- Let an owned DX11 sender select GPU keyed sharing before initialization. Detailed send
  admission has zero timeout; Busy queues no shared-image copy or counter advance. The original
  Boolean send API retains its timeout. There is no automatic downgrade to CPU-only ownership.
- Rebuild the native archive when SDK sources or headers change, alongside the original shim
  invalidation rules. Rust's caller-selected CRT and Windows/MSVC build constraints are retained.

Counter availability is separate from the runtime's local publication sequence and from GPU
completion. The SDK's signed counter is bounded; the extension does not invent monotonic wrap
semantics. Known versioned capability is an endpoint-owner contract for every replacement,
not something inferred from a sender name, semaphore existence or a positive counter value.

## GPU and metadata ownership

Golden's owned outputs opt into the SDK's existing keyed-texture allocation path, keeping the
standard Spout 32-bit legacy share-handle representation. Peers must support D3D11 keyed shared
textures; DX9 texture interoperability is outside this route. Ordinary unversioned/legacy
incoming senders remain supported. Sharing and counter modes are separate instance choices,
with no registry writes or changes to other applications' settings.

The named CPU lock protects counter decrement/restore and copy enqueueing. Keyed transfers
acquire that metadata lock before GPU ownership and release both after enqueueing. Counter
peek uses only the CPU lock, so unchanged polling does not fence or monopolize the GPU image.
Detailed transfers still acquire the keyed mutex before touching shared pixels. Zero-timeout
send admission preserves healthy consumers when a peer retains GPU ownership.

A CPU mutex and `Flush` do not establish GPU completion:
[Microsoft's Flush contract](https://learn.microsoft.com/en-us/windows/desktop/api/d3d11/nf-d3d11-id3d11devicecontext-flush)
is asynchronous. Shared-image rendering access uses
[IDXGIKeyedMutex](https://learn.microsoft.com/windows/win32/api/dxgi/nn-dxgi-idxgikeyedmutex).
The native regression isolates direct receive, the keyed source bridge, the full renderer and
test-only GPU input/output taps. One final readback checks exact marker/version lineage;
normal rendering adds no CPU pixel reads. A test-owned legacy named sender separately preserves
the static foreign/counter/Busy cases. Full current acceptance remains in the product ledger.

[PATCHES.json](PATCHES.json) records the resulting local file hashes and affected paths after
validation. Source transfer, reconnect, resize, foreign/unversioned behavior and GPU ownership
are verified through Golden Video's adjacent tests and MapGyver's owned native controls.
Current acceptance and remaining qualification belong to
[the MapGyver ledger](../../docs/progress/mapgyver-status.md).
