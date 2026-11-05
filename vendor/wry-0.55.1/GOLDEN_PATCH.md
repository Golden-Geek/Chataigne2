# Wry Windows teardown backport

This is the library source of crates.io `wry` 0.55.1, under its original Apache-2.0/MIT
licenses. The shared desktop host's Tauri 2.11 minor uses this Wry API. A reproduced
MapGyver normal shutdown aborted at `src/webview2/mod.rs:1256`: a focus message
re-entered the parent subclass after its controller reference data had been cleared.

The runtime change backports [upstream PR #1795](https://github.com/tauri-apps/wry/pull/1795)
(release commit `3fbf592`): remove the parent subclass before dropping its boxed
controller reference. Later focus/move/size messages then reach the remaining window
procedure, with no callback pointing at cleared/freed data. App code adds no message
filter or lifecycle workaround.

`src/webview2/tests/mod.rs` exercises the real WebView2 parent callback and a lower
subclass that synchronously re-enters focus/move handling during detach. It verifies
callback removal and repeated detach, then normally drops the WebView and HWND.
Run its ignored native test only after software/compile prerequisites pass.

Use `tests/run-windows.ps1 -ManifestTool <Windows-SDK-x64-mt.exe>` for the software
tests, then add `-Native` after prerequisite checks pass. The standalone harness
embeds `tests/windows.manifest`, selecting the same Common Controls v6 assembly
as the Tauri application; otherwise `GetWindowSubclass` has no named export in
the legacy default assembly and the harness cannot load. This changes only the
generated test executable, not OS settings or production DLL loading.

The package retains upstream runtime sources, build script, README and licenses.
The normalized manifest omits unpackaged examples and their development-only
dependencies; Tao remains for its existing `x11` feature forwarding, and base64/dom_query/sha2
remain for upstream library unit tests. Library runtime dependencies/features are unchanged.
The workspace lockfile is authoritative for production. The excluded vendor package's native
unit tests use `tests/Cargo.lock`, copied to the ignored package lock by the runner;
both exercise WebView2 0.38.2/windows 0.61.3. The separate test lock does not resolve
or change production dependencies.
Upstream layout/formatting and long platform implementation files are intentionally
preserved for reviewable comparison; new tests live in their adjacent `tests/` directory.

Remove this patch when upgrading the shared host to a compatible Tauri/Wry pair
that includes #1795, and rerun this regression and normal native app shutdown.
