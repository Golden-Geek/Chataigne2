# Release readiness

The root package command owns desktop packaging:

```text
npm ci
npm run package
```

It builds the production UI and invokes the workspace-pinned Tauri CLI against
`apps/chataigne/tauri.conf.json`. Use `npm run package:check` to compile the release application
without producing installers. Platform bundles are created on their native runners by
`.github/workflows/release.yml` and uploaded as workflow artifacts.

Release qualification always uses unsigned native packages. Certificates, notarization credentials,
and timestamp services are launch-time distribution concerns; they never block validation,
cross-platform package smoke, or Sound Card completion.

When a launch candidate is ready to publish, signing can be enabled separately with
`GC_REQUIRE_SIGNING=1` and the platform credentials:

- Windows: import the certificate into `Cert:\CurrentUser\My`, then provide
  `WINDOWS_CERTIFICATE_THUMBPRINT` and the issuer-approved `WINDOWS_TIMESTAMP_URL`. The checked-in
  Tauri sign command invokes `tools/release/sign-windows.ps1` for every bundle artifact.
- macOS: provide `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, and either the Apple ID
  notarization trio (`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`) or the App Store Connect API
  pair (`APPLE_API_KEY`, `APPLE_API_ISSUER`). Tauri performs signing, notarization, and stapling.
- Linux: provide `SIGN_KEY` for AppImage signing. RPM signing can additionally use
  `TAURI_SIGNING_RPM_KEY` and `TAURI_SIGNING_RPM_KEY_PASSPHRASE` when RPM is selected.

Secrets never belong in repository configuration or generated artifacts. An unsigned validation
candidate is not qualified until its native bundle is installed on a clean environment, launched,
connected to the engine, exercised through the canonical save/reload workflow, and removed cleanly.
Signing and notarization are required only for the later act of public distribution.

The native workflow performs that qualification with:

```text
python tools/qualification/package_smoke.py \
  --platform <windows|macos|linux> \
  --output-dir target/qualification/package/<platform>
```

Windows installs the produced NSIS package into an isolated location and invokes its uninstaller.
macOS runs the executable inside the produced app bundle. Linux runs the produced AppImage with
extract-and-run isolation. Every platform launches the packaged binary in headless host mode, then
uses a real browser to connect, load a fixture, mutate the outliner and inspector, exercise Formula
and State Machine graphs, save/reopen, verify live feedback, create a new project, and clean its
isolated app data.

The release soak is separate and defaults to five minutes:

```text
python tools/qualification/soak.py \
  --output-dir target/qualification/soak/release-candidate
```

It keeps at least three independent browser clients connected while rotating authoritative
mutations and verifying observation fan-out, save/reload, WebSocket traffic, browser errors, and
cleanup. It samples each browser heap, requires a stable plateau, records runtime queue depth and
peak, requires queues to drain, rejects host failure markers, and repeats the full Chataigne
hardware-simulator suite three times. The `--allow-short` option exists only for runner development
and is not a release qualification. Longer endurance runs remain available through
`--duration-seconds` for release candidates.
