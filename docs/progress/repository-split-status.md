# Repository split status

**Updated 26 November 2026 · `████░░░░░░` 40% accepted (2/5 gates)**

**Current:** MapGyver2 installer verification after the local standalone runtime checks. **Next:** finish the Rust library suite and verify the release-mode installer matrix before promoting MapGyver2. **Blocker:** none.

| Gate | State | Progress | Evidence or return action |
| --- | --- | --- | --- |
| Source and ownership | Accepted | `██████████` 100% (1/1) | [Source refs](#source-refs) and [ownership map](../plan/Repository_Split_Implementation_Plan.md#ownership). |
| Standalone Golden Core | Accepted | `██████████` 100% (1/1) | All seven [standalone CI jobs](https://github.com/Golden-Geek/GoldenCore/actions/runs/45787206204) passed at `d2b7e296`. Its publication on `main` was verified. |
| Standalone MapGyver2 | Active | `░░░░░░░░░░` 0% (0/1) | Local runtime checks passed at [a96896d7](https://github.com/Golden-Geek/MapGyver2/commit/a96896d721c72b4d38113ff69bb80e1d699ad489). Finish the installer matrix and verify the default branch. |
| Chataigne2 cutover | Pending | `░░░░░░░░░░` 0% (0/1) | Switch to the verified Golden pin after MapGyver2 qualification. |
| Independent pin upgrades | Waiting | `░░░░░░░░░░` 0% (0/1) | Verify both consumers and test a Golden revision bump. |

Progress counts the five fixed migration gates in the [implementation plan](../plan/Repository_Split_Implementation_Plan.md). Product hardware and performance gates remain in the [Chataigne ledger](implementation-status.md) and [MapGyver ledger](mapgyver-status.md).

## Source refs

- `Golden-Geek/Chataigne2` `mapgyver` is `3b7a4bbcbfc211903a52d43f31d3fe5e17e73ac3`. `main` is `40acd67df94920b63469a77da0ee64e40368d60b` and is its ancestor. The source is 831 commits ahead of `main` and contains Chataigne changes. The local checkout matched the live remote tips.
- `Golden-Geek/MapGyver2` `main` is `908dbfdfc93a678c7c33c6570463ce11c2f57f9a`, the license commit. The extracted MapGyver history will join that ancestry.
- Worktree `D:/Projects/Dev/Chataigne2-repository-split` holds `migration/repository-split`. Historical MapGyver beta tags `1.0.0b1` through `1.0.0b4` remain in Chataigne2. GitHub release lookups confirmed the existing downloads for [b1 (two assets)](https://github.com/Golden-Geek/Chataigne2/releases/tag/1.0.0b1) and [b4 (eight assets)](https://github.com/Golden-Geek/Chataigne2/releases/tag/1.0.0b4). The b2 and b3 tags have no GitHub release entry.
- The shared project is named **Golden Core** and its public destination is [Golden-Geek/GoldenCore](https://github.com/Golden-Geek/GoldenCore). The filtered import has 988 commits before standalone wiring. Its shared crate, package, and vendor trees matched the source trees byte-for-byte at the import commit.

Golden Core closed at `d2b7e296` with Windows, macOS and Linux Rust and audio coverage and shared UI and generated-contract checks. The [separate engine benchmark](https://github.com/Golden-Geek/GoldenCore/actions/runs/45782670420) produced samples. The inherited performance baseline still needs a matching environment fingerprint.

## Current batch

MapGyver2 imported 995 filtered commits, retained the destination license commit as an ancestor and pins Golden Core at `d2b7e296`. The fresh recursive HTTPS clone passed locked Cargo metadata, `npm ci`, UI checks and the product UI build. The independent checkout also passed strict MapGyver Clippy, watcher tests and lint, 397 Python tool tests, the probe UI build and contract check, and the pinned FFmpeg SDK check. The library suite remains live in session `66466`.

The clean clone at `a96896d7` used its own build directory after the original app build hit the test runner's open FFmpeg DLLs. Its normal Windows binary built successfully. Headless authoring, Save/New/Load, browser GPU Media Preview, Screen Warping and native WebView2 readiness passed. Loading the pre-split Composition review copy preserved the three instance IDs, its 320 × 240 canvas and SHA-256 `c6c87140dd17c00f47b3b45de976f0ee80d8edfa115be6c8e48ee075da8d3408`. The broader Composition UI scenario stopped at its source-channel assertion before File Open. The portable Screen observer found no windows by title on Windows x64. The Windows output-class observer then measured red and green at the authored bounds on the same owned HWND, and verified disable and reopen through ordinary Screen operations. Both clean-clone Git trees stayed clean.

The initial [desktop run](https://github.com/Golden-Geek/MapGyver2/actions/runs/45797348707) passed macOS ARM64. Linux x64 built the `.deb` and hit an Ubuntu mirror checksum mismatch during the clean install. The installer workflow fix enables branch verification in release mode including Windows x64, drops the unused ASIO setup on Windows and includes the pinned Golden checkout in the Debian ARM archive. Local reconstruction of that archive passed locked metadata and `npm ci`. Follow the [replacement desktop](https://github.com/Golden-Geek/MapGyver2/actions/runs/45798526192) and [Windows ARM64](https://github.com/Golden-Geek/MapGyver2/actions/runs/45798526156) runs. Windows ARM64 passed both SDK steps. On [Linux ARM](https://github.com/Golden-Geek/MapGyver2/actions/runs/45798526146), ARM64 passed clean install, workbench, media and output. Armhf built, passed clean install and uploaded the package. The emulated native WebKit window mapped but failed to request its workbench snapshot. Native 32-bit desktop, media and output remain open in the [MapGyver product gate](mapgyver-status.md). The tag workflow remains the only release publisher. Stop adding scope at 11:31 Europe/Paris on 26 November 2026 and retain live checks for the next turn.
