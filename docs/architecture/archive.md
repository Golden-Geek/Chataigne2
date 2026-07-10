# Pre-monorepo archive map

The old repositories are intentionally absent from the production tree. Their final
imported states remain recoverable from Git history without keeping a second implementation
path:

| Former repository | Imported source commit | Monorepo import commit |
|---|---|---|
| `golden_core` | `b3453b4aa6338f7ea22084e6813f43de1a1f9c25` | `bcd1c3f` |
| `golden_alchemist_core` | `c1b0205c67c22e005cfa0943fa416c5fd3d85158` | `ed7ff14` |
| `golden_ui` | `def1d5af3850b14ee2b67f94b689e994e45bedab` | `4c2a25e` |
| `golden_alchemist_ui` | `b4b9fe6fa06c328e7bc0f5b487a3779320b6666f` | `54b6d41` |

The pre-rewrite Chataigne shell, runtime, UI, Tauri configuration, generated protocol, and
fixtures remain in commits before Phase 1 (`687afe9`). No archived source is compiled,
packaged, indexed as a workspace layer, or loaded at runtime.

