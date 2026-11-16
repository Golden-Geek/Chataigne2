# Composition native raster acceptance

MapGyver owns the authored scene, intents and preview source keys. The fixture uses the public
ChannelRenderer and production PreviewController/PreviewLayout compositor; Golden owns D3D11,
Spout delivery and finite SDK readbacks. No private package files or alternate renderer are used.

Run the software oracle checks and compile the native fixture first:

```text
cargo test --locked -p MapGyver --features native-qualification --test composition_rendering
cargo clippy --locked -p MapGyver --features native-qualification --test composition_rendering -- -D warnings
```

Then run the explicit finite hardware test:

```text
cargo test --locked -p MapGyver --features native-qualification --test composition_rendering native_nested_composition_pixels -- --ignored --nocapture
```

It checks both available hardware adapters, twenty-seven scene states and three SDK consumer/device
readbacks per state: two outputs share the same root, and the third receives the production
preview compositor. The preview pass uses the ordinary host's linear premultiplied RGBA8 format.
The CPU oracle independently inverse-maps the authored transforms and evaluates coverage/blends.
The added states collapse either/both axes, include signed zero, and restore mirrored/original
scale while healthy siblings continue through independent outputs and the preview compositor.
An all-collapsed root then verifies empty-output clearing and complete restoration.
Media metadata disable/restore states exercise the same authored enable state used by the
Inspector and public Composition operation, while sibling References retain their shared source.
Front/Back, arrangement Undo/Redo and Reset/Fit/Fill add eight states. The independent oracle
paints authored children in reverse so the first child is on top, including in nested content;
the backend's layer-generation code is not used to compute expected order or geometry.

The shared oracle and owned native fixture live in `../support`. Each intermediate UNORM pass
propagates the legal integer range from
[D3D11 §3.2.3.6](https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#3.2.3.6%20FLOAT%20-%3E%20UNORM)
through compositing, opacity and resampling before sRGB encoding. Report both nominal error and residual error outside that range; the latter
must stay within two output bytes. Near-black nominal differences can exceed that residual bound.
The focused oracle test preserves the discovered NVIDIA example and the exact allowed interval.

Every renderer retires before its native owner inventory is checked empty; readback payload pools
must return idle. A 120-second watchdog fails stuck retirement rather than pretending cleanup.

These checks qualify Composition raster content. They do not observe an on-screen editor,
gestures, physical alignment/DPI, scanout/cadence or driver allocation growth. Those remain with
M06's editor gate, M05's presentation matrix and M03's resource/performance qualification.
