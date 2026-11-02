use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! command_kinds {
    ($($kind:ident => $label:literal, $group:literal, [$($field:literal),*];)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
        pub enum MediaCommandKind { $($kind),* }
        impl MediaCommandKind {
            pub const ALL: &'static [Self] = &[$(Self::$kind),*];
            pub fn key(self) -> &'static str { match self { $(Self::$kind => stringify!($kind)),* } }
            pub fn label(self) -> &'static str { match self { $(Self::$kind => $label),* } }
            pub fn group(self) -> &'static str { match self { $(Self::$kind => $group),* } }
            pub fn fields(self) -> &'static [&'static str] { match self { $(Self::$kind => &[$($field),*]),* } }
        }
    };
}
command_kinds! {
    CreateReference => "Create Reference", "Media", ["reference_target", "label"];
    ConfigureReference => "Set Reference Target", "Media", ["media", "reference_target"];
    CreateMedia => "Create Media", "Media", ["media_kind", "label"];
    CreateMediaInFolder => "Create Media in Folder", "Media", ["target_folder", "media_kind", "label"];
    RenameMedia => "Rename Media", "Media", ["media", "label"];
    RemoveMedia => "Remove Media", "Media", ["media"];
    ConfigureResolution => "Set Resolution", "Media", ["media", "resolution_enabled", "resolution"];
    ConfigureSampling => "Set Sampling", "Media", ["media", "sampling"];
    AddChannel => "Add Variant", "Channels", ["media", "label"];
    RenameChannel => "Rename Variant", "Channels", ["media", "channel", "label"];
    ReorderChannel => "Reorder Variant", "Channels", ["media", "channel", "after_channel"];
    MoveChannel => "Move Variant", "Channels", ["media", "channel", "direction"];
    RemoveChannel => "Remove Variant", "Channels", ["media", "channel"];
    SolidColor => "Set Solid Color", "Channels", ["media", "channel", "rgba"];
    Picture => "Set Picture Asset", "Channels", ["media", "channel", "asset"];
    SharedTexture => "Set Shared Texture Sender", "Channels", ["media", "channel", "sender"];
    ImportPicture => "Import Picture", "Assets", ["locator_kind", "project_path", "absolute_path", "media", "channel", "label"];
    ImportAsset => "Import Asset", "Assets", ["locator_kind", "project_path", "absolute_path", "label"];
    RelinkAsset => "Relink Asset", "Assets", ["asset", "locator_kind", "project_path", "absolute_path"];
    ReloadAsset => "Reload Asset", "Assets", ["asset"];
    RemoveAsset => "Remove Asset", "Assets", ["asset"];
    CreateFolder => "Create Folder", "Folders", ["target_folder", "label"];
    RenameFolder => "Rename Folder", "Folders", ["target_folder", "label"];
    RemoveFolder => "Remove Folder", "Folders", ["target_folder"];
    MoveEntry => "Move Media or Folder", "Folders", ["entry", "target_folder"];
    InspectManager => "Inspect Media", "Queries", [];
    InspectChannel => "Inspect Variant", "Queries", ["media", "channel"];
    InspectChannels => "Inspect Media Channels", "Queries", ["media", "start", "count"];
    InspectAsset => "Inspect Asset", "Queries", ["asset"];
    InspectChoices => "Inspect Channel Choices", "Queries", ["choice_scope", "search", "start", "count"];
    CreateScript => "Create Media Script", "Scripts", ["label"];
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MediaCommandCatalogItem {
    pub kind: MediaCommandKind,
    pub label: String,
    pub group: String,
}

pub(crate) fn catalog() -> Vec<MediaCommandCatalogItem> {
    MediaCommandKind::ALL
        .iter()
        .map(|kind| MediaCommandCatalogItem {
            kind: *kind,
            label: kind.label().into(),
            group: kind.group().into(),
        })
        .collect()
}
