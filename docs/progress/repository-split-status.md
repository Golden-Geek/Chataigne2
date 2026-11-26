# Repository split status

**Updated 26 November 2026 · `████░░░░░░` 40% accepted (2/5 gates)**

**Current:** standalone MapGyver2 verification. **Next:** finish the local Rust suite and verify the release-mode installer matrix. **Blocker:** none.

| Gate | State | Progress | Evidence or return action |
| --- | --- | --- | --- |
| Source and ownership | Accepted | `██████████` 100% (1/1) | [Source refs](#source-refs) and [ownership map](../plan/Repository_Split_Implementation_Plan.md#ownership). |
| Standalone Golden Core | Accepted | `██████████` 100% (1/1) | All seven [standalone CI jobs](https://github.com/Golden-Geek/GoldenCore/actions/runs/45787206204) passed at `d2b7e296`. Its publication on `main` was verified. |
| Standalone MapGyver2 | Active | `░░░░░░░░░░` 0% (0/1) | [Standalone checkpoint](https://github.com/Golden-Geek/MapGyver2/commit/27305b17a475b987b4f85491c3620da9a4d422c1). Complete clean-clone, runtime and installer verification. |
| Chataigne2 cutover | Pending | `░░░░░░░░░░` 0% (0/1) | Switch to the verified Golden pin after MapGyver2 qualification. |
| Independent pin upgrades | Waiting | `░░░░░░░░░░` 0% (0/1) | Verify both consumers and test a Golden revision bump. |

Progress counts the five fixed migration gates in the [implementation plan](../plan/Repository_Split_Implementation_Plan.md). Product hardware and performance gates remain in the [Chataigne ledger](implementation-status.md) and [MapGyver ledger](mapgyver-status.md).

## Source refs

- `Golden-Geek/Chataigne2` `mapgyver` is `3b7a4bbcbfc211903a52d43f31d3fe5e17e73ac3`. `main` is `40acd67df94920b63469a77da0ee64e40368d60b` and is its ancestor. The source is 831 commits ahead of `main` and contains Chataigne changes. The local checkout matched the live remote tips.
- `Golden-Geek/MapGyver2` `main` is `908dbfdfc93a678c7c33c6570463ce11c2f57f9a`, the license commit. The extracted MapGyver history will join that ancestry.
- Worktree `D:/Projects/Dev/Chataigne2-repository-split` holds `migration/repository-split`. Historical MapGyver beta tags `1.0.0b1` through `1.0.0b4` remain in Chataigne2. GitHub release lookups confirmed the existing downloads for [b1 (two assets)](https://github.com/Golden-Geek/Chataigne2/releases/tag/1.0.0b1) and [b4 (eight assets)](https://github.com/Golden-Geek/Chataigne2/releases/tag/1.0.0b4). The b2 and b3 tags have no GitHub release entry.
- The shared project is named **Golden Core** and its public destination is [Golden-Geek/GoldenCore](https://github.com/Golden-Geek/GoldenCore). The filtered import has 988 commits before standalone wiring. Its shared crate, package, and vendor trees matched the source trees byte-for-byte at the import commit.

## Current batch

The standalone workspace resolves 22 shared crates and all three reviewed vendor patches. The fresh HTTPS clone at `f7d6c3b` passed locked metadata, `npm ci`, UI checks and 570 unit tests, and stayed clean. Dashboard, video preview, formatting, Windows Clippy, and all four shared generators passed. [Hosted UI and contracts](https://github.com/Golden-Geek/GoldenCore/actions/runs/45780323572/job/139774069717) also passed.

The full local Windows Rust workspace suite passed at `49aa6672` with D3D11, NDI and browser preview enabled. The checkout stayed clean. The same revision passed hosted workspace Clippy on Linux, macOS and Windows, and passed hosted UI, generated contracts and audio on all three platforms, including the external Windows audio consumer.

The [Linux audio job](https://github.com/Golden-Geek/GoldenCore/actions/runs/45782670399/job/139781828098) passed all 194 tests, including the Null soak that previously failed because the runner denied realtime thread priority. The diagnostic remains in reports and is excluded from acceptance only for the synthetic Null backend. The [separate benchmark](https://github.com/Golden-Geek/GoldenCore/actions/runs/45782670420) produced raw samples, but the inherited baseline lacks a matching environment fingerprint. A separately qualified baseline is required for performance comparison.

Golden Core closed at `d2b7e296`. The exact published revision passed Rust workspace tests, strict Clippy and audio checks on Windows, macOS and Linux, plus UI type, unit, browser and generated-contract checks. The earlier Linux and macOS process-error failure was resolved by checking that ownership transfers preserve the raw OS error.

MapGyver2 imported 995 filtered commits and retained the destination license commit as an ancestor. Its standalone workspace pins Golden Core at `d2b7e296`. A fresh recursive HTTPS clone of `27305b17` passed locked Cargo metadata, `npm ci`, UI checks and the product UI build, and stayed clean. The independent checkout also passed strict MapGyver Clippy, watcher tests and lint, 397 Python tool tests, probe UI build and contract verification, and the pinned FFmpeg SDK check. The local library suite remains live in session `66466`.

The first [desktop run](https://github.com/Golden-Geek/MapGyver2/actions/runs/45797348707) skipped the tag-only Windows x64 installer. [Windows ARM64](https://github.com/Golden-Geek/MapGyver2/actions/runs/45797348725) verified the SDK and then stopped at a Chataigne-only ASIO helper. [Linux ARM](https://github.com/Golden-Geek/MapGyver2/actions/runs/45797348770) could not build the UI because its archived input omitted the Golden submodule.

[Installer workflow fix](https://github.com/Golden-Geek/MapGyver2/commit/a96896d721c72b4d38113ff69bb80e1d699ad489) adds branch release-mode verification including Windows x64, uses safe artifact names, removes the unneeded ASIO setup and includes the exact Golden commit in the ARM archive. Reconstructing that archive locally passed locked metadata and `npm ci`. Workflow trigger checks confirmed that only the tag workflow can publish releases. Follow the replacement [desktop](https://github.com/Golden-Geek/MapGyver2/actions/runs/45798526192), [Windows ARM64](https://github.com/Golden-Geek/MapGyver2/actions/runs/45798526156) and [Linux ARM](https://github.com/Golden-Geek/MapGyver2/actions/runs/45798526146) runs. The local app binary build is live in session `70113`. Run the isolated headless product check after it succeeds. Stop adding scope at 11:31 Europe/Paris on 26 November 2026.
