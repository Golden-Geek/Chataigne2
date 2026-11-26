# Repository split status

**Updated 26 November 2026 · `██░░░░░░░░` 20% accepted (1/5 gates)**

**Current:** Golden Core extraction. **Next:** build the filtered Golden tree as its own Cargo and npm workspace. **Blocker:** none.

| Gate | State | Progress | Evidence or return action |
| --- | --- | --- | --- |
| Source and ownership | Accepted | `██████████` 100% (1/1) | [Source refs](#source-refs) and [ownership map](../plan/Repository_Split_Implementation_Plan.md#ownership). |
| Standalone Golden Core | Active | `░░░░░░░░░░` 0% (0/1) | Produce and verify a clean standalone Golden Core clone. |
| Standalone MapGyver2 | Pending | `░░░░░░░░░░` 0% (0/1) | Import product history and verify against a published Golden commit. |
| Chataigne2 cutover | Pending | `░░░░░░░░░░` 0% (0/1) | Switch to the verified Golden pin after MapGyver2 qualification. |
| Independent pin upgrades | Waiting | `░░░░░░░░░░` 0% (0/1) | Verify both consumers and test a Golden revision bump. |

Progress counts the five fixed migration gates in the [implementation plan](../plan/Repository_Split_Implementation_Plan.md). Product hardware and performance gates remain in the [Chataigne ledger](implementation-status.md) and [MapGyver ledger](mapgyver-status.md).

## Source refs

- `Golden-Geek/Chataigne2` `mapgyver` is `3b7a4bbcbfc211903a52d43f31d3fe5e17e73ac3`. `main` is `40acd67df94920b63469a77da0ee64e40368d60b` and is its ancestor. The source is 831 commits ahead of `main` and contains Chataigne changes. The local checkout matched the live remote tips.
- `Golden-Geek/MapGyver2` `main` is `908dbfdfc93a678c7c33c6570463ce11c2f57f9a`, the license commit. The extracted MapGyver history will join that ancestry.
- Worktree `D:/Projects/Dev/Chataigne2-repository-split` holds `migration/repository-split`. Historical MapGyver beta tags `1.0.0b1` through `1.0.0b4` remain in Chataigne2. The application working branch remains available for product development.
- The shared project is named **Golden Core** and uses `Golden-Geek/GoldenCore`. Repository lookup confirmed the slug is unused and the local GitHub identity has organization administration access.

## Current batch

The source and ownership gate closed with live ref, ancestry, and destination checks. The next implementation batch belongs to Golden Core and delivers an app-independent Cargo and npm checkout. Start from the recorded source in a disposable clone, verify retained paths against the source, then run locked workspace metadata and `npm ci`. Stop adding work at 09:40 Europe/Paris on 26 November 2026. Shared test, lint, codegen and clean-clone qualification remain required before this gate can close.
