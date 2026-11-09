# Native qualification ownership

Golden owns D3D11 completion, device generations and native source/output resources.
MapGyver's fixtures exercise the real app engine, File workers and renderer with public
Golden APIs. Browser fixtures separately exercise the ordinary bundled Tauri host.
Finite Spout readback is an independent test observer; production Video stays GPU-resident
where supported. Functional pixel checks do not establish sustained performance.

`invoke-device-removal.ps1` controls a fresh owned test process. It requires an exact live-PID
readiness marker, retains process handles/exit codes and records its result directory under
`target/m034-removal-*`. The historical directory prefix is shared by both source fixtures.
`-SourceKind SharedTexture` selects the original Golden test; `-SourceKind Video` selects
MapGyver's eight-source, both-GPU Video fixture. The default action performs no reset.

The Video test executable requires default features plus `native-qualification`. Build through
the [pinned SDK helper](../../../../tools/mapgyver/ffmpeg/README.md), then stage the compiled
test executable and exact pinned DLLs in an isolated directory. The current prepared executable
and its hash/checks are recorded in the [owning receipt](../../../../docs/progress/evidence/mapgyver-m10-device-preflight-2026-11-09.json).

From the repository root, the prepared no-reset preflight is:

```powershell
./apps/mapgyver/tests/native/invoke-device-removal.ps1 -TestExecutable ./target/mapgyver-m10-device-ready/MapGyverTests.exe -SourceKind Video
```

After software/strict checks and that preflight succeed, the already-authorized genuine test
can run from an elevated Windows PowerShell:

```powershell
./apps/mapgyver/tests/native/invoke-device-removal.ps1 -TestExecutable ./target/mapgyver-m10-device-ready/MapGyverTests.exe -SourceKind Video -AllowDriverReset -UseCurrentToken
```

`-UseCurrentToken` invokes installed DXCap with the caller's token and redirected logs. It
requests no elevation prompt; a standard Windows token is rejected by DXCap. Without this
switch, the controller's existing interactive mode uses `RunAs`. Driver reset reaches other
GPU apps and may briefly blank displays; save their work before running it.

The fixture preserves Playing/Loop settings and exact authored JSON/history across recovery,
then explicitly Pauses before comparing static seek images. Seek itself preserves playback.
A genuine pass requires a changed renderer device generation, restored native hardware/BC
routes, fresh output pixels and complete retirement within the existing five-second shutdown
target. An unaffected adapter supplies healthy-control evidence only. A denied trigger or a
Running label alone cannot qualify recovery.
