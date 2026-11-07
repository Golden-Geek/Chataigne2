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

`ReloadPicture` advances only the selected channel's runtime reload generation
and invalidates its capture guard. The ordinary preview worker wakes through
that guard's existing subscription; no authored edit, polling or history step is
needed. Capture carries this generation into frozen playing sources. Preparation
compares generations for unchanged locator bindings and replaces that shared
file group once, retaining unrelated images. The Inspector's Reload action,
typed command and `reloadPicture` script method use this same intent. Project
load creates fresh runtime generations and never replays Reload.

Preparation remains on Golden Assets workers, with existing source/decode/cache
limits, scope cancellation and retirement barriers. Playing owned Matrix media
retain their frozen File definition after authored removal; releasing the last
runtime consumer retires its preparation group. Runtime retry targets a channel
reference, not a catalog Asset. Explicit standalone catalog Asset operations
remain separate and cannot retarget Pictures.

The [status ledger](../progress/mapgyver-status.md) owns current M13/9 acceptance
and links its software, bundled authoring and native pixel/retirement evidence.
