# Repository split status

**Updated 26 November 2026 · `██░░░░░░░░` 20% accepted (1/5 gates)**

**Current:** Golden Core standalone verification. **Next:** verify the Null audio soak fix and the remaining hosted Rust tests. **Blocker:** none.

| Gate | State | Progress | Evidence or return action |
| --- | --- | --- | --- |
| Source and ownership | Accepted | `██████████` 100% (1/1) | [Source refs](#source-refs) and [ownership map](../plan/Repository_Split_Implementation_Plan.md#ownership). |
| Standalone Golden Core | Active | `░░░░░░░░░░` 0% (0/1) | [Current Golden Core revision](https://github.com/Golden-Geek/GoldenCore/commit/49aa6672e840b4d78ca384c9aed6076c992934cc). Finish hosted Rust and audio verification. |
| Standalone MapGyver2 | Pending | `░░░░░░░░░░` 0% (0/1) | Import product history and verify against a published Golden commit. |
| Chataigne2 cutover | Pending | `░░░░░░░░░░` 0% (0/1) | Switch to the verified Golden pin after MapGyver2 qualification. |
| Independent pin upgrades | Waiting | `░░░░░░░░░░` 0% (0/1) | Verify both consumers and test a Golden revision bump. |

Progress counts the five fixed migration gates in the [implementation plan](../plan/Repository_Split_Implementation_Plan.md). Product hardware and performance gates remain in the [Chataigne ledger](implementation-status.md) and [MapGyver ledger](mapgyver-status.md).

## Source refs

- `Golden-Geek/Chataigne2` `mapgyver` is `3b7a4bbcbfc211903a52d43f31d3fe5e17e73ac3`. `main` is `40acd67df94920b63469a77da0ee64e40368d60b` and is its ancestor. The source is 831 commits ahead of `main` and contains Chataigne changes. The local checkout matched the live remote tips.
- `Golden-Geek/MapGyver2` `main` is `908dbfdfc93a678c7c33c6570463ce11c2f57f9a`, the license commit. The extracted MapGyver history will join that ancestry.
- Worktree `D:/Projects/Dev/Chataigne2-repository-split` holds `migration/repository-split`. Historical MapGyver beta tags `1.0.0b1` through `1.0.0b4` remain in Chataigne2. The application working branch remains available for product development.
- The shared project is named **Golden Core** and its public destination is [Golden-Geek/GoldenCore](https://github.com/Golden-Geek/GoldenCore). The filtered import has 988 commits before standalone wiring. Its shared crate, package, and vendor trees matched the source trees byte-for-byte at the import commit.

## Current batch

The standalone workspace resolves 22 shared crates and all three reviewed vendor patches. The fresh HTTPS clone at `f7d6c3b` passed locked metadata, `npm ci`, UI checks and 570 unit tests, and stayed clean. Dashboard, video preview, formatting, Windows Clippy, and all four shared generators passed. [Hosted UI and contracts](https://github.com/Golden-Geek/GoldenCore/actions/runs/45780323572/job/139774069717) also passed.

The full local Windows Rust suite passed on the original standalone revision. Focused Windows desktop Clippy and hosted macOS Clippy passed after gating Windows-only preview code and fixing the refresh-rate lint. Hosted UI and macOS audio also passed on the revision with those fixes.

The [serial Linux audio run](https://github.com/Golden-Geek/GoldenCore/actions/runs/45781382140/job/139777475765) identified realtime thread priority denial as the only soak failure. The narrow fix retains that diagnostic in reports and excludes it from acceptance only for the synthetic Null backend. All real-device warnings still fail acceptance. Local formatting, the three device-soak tests and strict Golden Audio Clippy passed. Verify the hosted matrix on the new revision before pinning it in MapGyver2. Stop adding scope at 09:40 Europe/Paris on 26 November 2026.
