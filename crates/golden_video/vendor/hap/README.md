# HAP reference dependency

Golden Video owns this codec boundary. The application supplies File demand and transport;
this code returns GPU-compressed blocks and never performs CPU texture/pixel expansion.

- `hap.c` and `hap.h`: [Vidvox reference source](https://github.com/Vidvox/hap), commit
  `d847f6bbd3be88575dd4ef33a877243780e3be76`, BSD-2-Clause notices retained in both files.
- `snappy-c.h`: [Google Snappy](https://github.com/google/snappy/blob/1.2.2/snappy-c.h),
  tag `1.2.2`, BSD-3-Clause notice retained. Its private-prefixed C entry points are implemented
  by pinned `snap=1.1.1`, not a second C++ Snappy dependency.
- Cargo pins `cc=1.11.1` and `bindgen=0.72.1`; bindings are generated from the public header.
  There is no network fetch or dependency source rewrite during the build.

The reference C file intentionally keeps upstream layout and exceeds the normal source-file
line limit. Local safety changes cover unsigned integer reads, overflow-safe section lengths,
complete nonempty chunk tables, bounded two-plane container traversal, compressed chunk ranges
and output slice admission before pointer arithmetic. These checks reject malformed packets;
codec/texture algorithms remain the reference implementation. Trailing whitespace is normalized
for repository checks. Rust additionally admits exact padded block storage, one supported
texture and at most1024 chunks before decode. Tests cover malformed offsets/lengths, expansion,
raw/Snappy blocks, odd dimensions, real MOV timing/seeks and retained frames.

The reference encoder is available only to explicit test fixture generation through its public
ABI. Runtime uses reference decode with synchronous bounded chunk work on the owning file
worker. The installed command-line encoder is not needed to build or play these fixtures.

The supported boundary is HAP BC1 RGB, HAP Alpha BC3 RGBA and HAP Q scaled-YCoCg BC3.
Golden's native publisher now uploads/samples these blocks on the selected GPU and applies
YCoCg conversion through its shared completion/retirement path. App File binding uses that same
clock-fenced loop. Complete M10/2 app/concurrency/interoperability acceptance remains required.
Unsupported texture variants reject explicitly; there is no software RGBA performance fallback.
Final bundled notices/source distribution and sustained qualification remain M10/4 requirements.
