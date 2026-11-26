# Repository split status

**Updated 26 November 2026 · `██░░░░░░░░` 20% accepted (1/5 gates)**

**Current:** Golden Core standalone verification. **Next:** inspect macOS and Windows workspace tests in the [candidate CI run](https://github.com/Golden-Geek/GoldenCore/actions/runs/45787206204). **Blocker:** none.

| Gate | State | Progress | Evidence or return action |
| --- | --- | --- | --- |
| Source and ownership | Accepted | `██████████` 100% (1/1) | [Source refs](#source-refs) and [ownership map](../plan/Repository_Split_Implementation_Plan.md#ownership). |
| Standalone Golden Core | Active | `░░░░░░░░░░` 0% (0/1) | [Candidate Golden Core revision](https://github.com/Golden-Geek/GoldenCore/commit/d2b7e296e817cb7ad17abb6b9bcf2abe345c9a7c). Finish hosted Rust verification. |
| Standalone MapGyver2 | Pending | `░░░░░░░░░░` 0% (0/1) | Import product history and verify against a published Golden commit. |
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

The original Linux and macOS workspace tests exposed a `golden_io` fixture that assumed raw OS code 5 had the same error kind everywhere. Windows passed the original workspace suite. The candidate tests preservation of the raw code across ownership transfers. All three focused local Windows tests passed. The [candidate Linux job](https://github.com/Golden-Geek/GoldenCore/actions/runs/45787206204/job/139796904444) passed the complete Rust workspace, including that test, and left the checkout clean. Candidate audio, UI, contracts and all three Clippy jobs passed. Candidate macOS and Windows workspace tests remain in progress. Follow those jobs and promote the passing commit to `main` before either app pins it. Stop adding scope at 10:25 Europe/Paris on 26 November 2026.
