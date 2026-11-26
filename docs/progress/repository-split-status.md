# Repository split status

**Updated 26 November 2026 · `██░░░░░░░░` 20% accepted (1/5 gates)**

**Current:** Golden Core standalone verification. **Next:** finish the Windows Rust suite and check hosted CI from a fresh checkout. **Blocker:** none.

| Gate | State | Progress | Evidence or return action |
| --- | --- | --- | --- |
| Source and ownership | Accepted | `██████████` 100% (1/1) | [Source refs](#source-refs) and [ownership map](../plan/Repository_Split_Implementation_Plan.md#ownership). |
| Standalone Golden Core | Active | `░░░░░░░░░░` 0% (0/1) | [Standalone workspace](https://github.com/Golden-Geek/GoldenCore/commit/f7d6c3b50051efa07d73da73a486a13d351927ed). Finish Rust and fresh-clone CI verification. |
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

The Golden Core batch delivered an independent Cargo/npm workspace and moved shared generators and fixtures into their owning repository. Locked metadata resolves 22 shared crates and all three reviewed vendor patches. A standalone `npm ci`, UI checks and 570 unit tests passed. The 20-case dashboard dense check, video preview lifecycle checks, pinned Rust formatting, Windows Clippy with D3D11/NDI/browser preview, and all four generated-contract checks passed. The source and ownership gate remains the only accepted gate until the standalone workspace Rust tests and fresh-checkout CI finish. Stop adding scope at 09:40 Europe/Paris on 26 November 2026 and retain any running validation for the next pass.
