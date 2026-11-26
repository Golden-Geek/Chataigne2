# Repository split status

**Updated 26 November 2026 · `██░░░░░░░░` 20% accepted (1/5 gates)**

**Current:** Golden Core standalone verification. **Next:** verify the portable lint and audio test changes in hosted CI and finish the Windows Rust suite. **Blocker:** none.

| Gate | State | Progress | Evidence or return action |
| --- | --- | --- | --- |
| Source and ownership | Accepted | `██████████` 100% (1/1) | [Source refs](#source-refs) and [ownership map](../plan/Repository_Split_Implementation_Plan.md#ownership). |
| Standalone Golden Core | Active | `░░░░░░░░░░` 0% (0/1) | [Standalone workspace](https://github.com/Golden-Geek/GoldenCore/commit/f7d6c3b50051efa07d73da73a486a13d351927ed) and [qualification fixes](https://github.com/Golden-Geek/GoldenCore/commit/4961c0abca712609543652141861463bdc48b44a). Finish Rust and fresh-clone CI verification. |
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

The first hosted matrix exposed unused Windows-only dispatch code on Linux and macOS, one macOS refresh-rate lint, and a Linux null-backend soak with a runtime warning. The qualification fix gates platform code at its call sites, preserves the test assertions, logs the underlying audio warning on failure, and runs hosted audio tests serially. Root and Golden Core formatting and diff checks passed for the fix. Windows workspace tests on the import (session `68684`) and focused Clippy on the fix (session `99442`) remain in progress. Verify the updated hosted matrix before pinning Golden Core in MapGyver2. Stop adding scope at 09:40 Europe/Paris on 26 November 2026.
