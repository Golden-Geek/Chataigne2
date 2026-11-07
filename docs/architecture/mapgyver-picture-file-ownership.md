# Picture File ownership

MapGyver Picture channels own their typed `File` Parameter. The generic Golden
Inspector edits that Parameter directly. `ConfigurePictureFile`, named script
functions, typed commands and targeted `ImportPicture` all edit the same owner;
none creates or forwards through a catalog Asset. Creating a Picture imports one
media subtree, and copying it copies the File value into independent Parameters.

`ChannelSource::Picture.file` projects the Parameter as a validated locator. The
channel schema is `mapgyver.media.channel.v2`; commands use
`mapgyver.media.command.v6`. Earlier schemas are rejected before Golden's
recoverable project decoding, rather than silently discarding Asset references.
The Rust DTOs generate the matching UI contracts.

The rendering boundary captures the File with the channel definition. Each
channel has an internal binding address; a bounded preparation table groups
equal locators within the renderer's project/root. A group owns its independent
logical source identity, preparation scope and decoded image lease. Thus one
owner changing File cannot reuse the identity of an image still used by a peer.
Equal Files share decode/upload resources without sharing authored Parameters.
Resource admission and image revision guards count unique shared images.

Preparation remains on Golden Assets workers, with existing source/decode/cache
limits, scope cancellation and retirement barriers. Playing owned Matrix media
retain their frozen File definition after authored removal; releasing the last
runtime consumer retires its preparation group. Runtime retry targets a channel
reference, not a catalog Asset. Explicit standalone catalog Asset operations
remain separate and cannot retarget Pictures.

M13/9 acceptance still requires the complete owning report in the
[status ledger](../progress/mapgyver-status.md). In particular, this ownership
slice does not yet provide an ordinary Inspector/command/script reload action
for replacing an already prepared file at the same path. That action and the
affected bundled/native fixtures must be completed before accepting the gate.
