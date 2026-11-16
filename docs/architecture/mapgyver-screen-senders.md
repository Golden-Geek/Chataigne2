# Screen sender names

MapGyver owns Screen naming policy in `apps/mapgyver/src/screens`. Golden supplies the
optional typed Parameter metadata and the native sender's exact-name, retry and retirement
contracts. The UI uses the generic Inspector; it has no sender-name policy or companion boolean.

`ScreenNode` declares one disableable String Parameter, **Sender name**. Its own enabled
metadata selects the mode independently of the Screen's effective enabled state. The String
retains its custom value while disabled. Fresh creation disables this Parameter before subtree
publication, including startup and New. Intrinsic factories keep their existing enabled baseline
so sparse loading preserves previously authored explicit names.

`ScreenNode::definition` reads the actual Parameter metadata from the document. The public
Rust-generated `ScreenTarget` distinguishes `AutomaticSpout { extent }` from
`Spout { sender, extent }`. This makes configuration round trips preserve automatic mode;
an unrelated FPS or quality edit cannot freeze the current derived name. Automatic configuration
leaves the dormant String untouched. Explicit configuration authors the String and enables it
in the same public operation/history transaction.

Output capture resolves automatic names as `MapGyver - <Screen label>`. Screen renames and
Sender name metadata changes invalidate the existing revision guards. Both modes share the
same exact-name collision domain. Disabled text is never a second endpoint authority.
Fullscreen reassignment preserves automatic mode and dormant text; its existing unique-name
fallback remains for explicit senders.

The resolved native binding remains Golden's existing Spout output. Name, target and project
changes use the existing supervised resource replacement and completion-safe retirement.
SDK rejection or suffix renaming is an error, with the existing retry/recovery behavior; names
are never silently truncated or changed to make allocation succeed.

Behavioral coverage lives under `screens/tests/sender_names.rs` and `screens/tests/scripting.rs`.
`tests/browser/sender-names.mjs` exercises the actual bundled Inspector and public transport,
with independent SDK discovery and exact consumer pixels. The owning acceptance report is
[M14 sender names](../progress/mapgyver-m14-sender-names.md).
