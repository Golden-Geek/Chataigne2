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

Local packages may be unsigned. Release qualification sets `GC_REQUIRE_SIGNING=1`, which makes the
preflight fail before compilation unless the platform credentials are present:

- Windows: import the certificate into `Cert:\CurrentUser\My`, then provide
  `WINDOWS_CERTIFICATE_THUMBPRINT` and the issuer-approved `WINDOWS_TIMESTAMP_URL`. The checked-in
  Tauri sign command invokes `tools/release/sign-windows.ps1` for every bundle artifact.
- macOS: provide `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, and either the Apple ID
  notarization trio (`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`) or the App Store Connect API
  pair (`APPLE_API_KEY`, `APPLE_API_ISSUER`). Tauri performs signing, notarization, and stapling.
- Linux: provide `SIGN_KEY` for AppImage signing. RPM signing can additionally use
  `TAURI_SIGNING_RPM_KEY` and `TAURI_SIGNING_RPM_KEY_PASSPHRASE` when RPM is selected.

Secrets never belong in repository configuration or generated artifacts. A release candidate is
not qualified until its native bundle is installed on a clean environment, launched, connected to
the engine, exercised through the canonical save/reload workflow, and removed cleanly.
