//! Rust-owned app intents and read/runtime projections; media settings persist in Parameter children.

pub use golden_assets::{AssetId, AssetLocator, AssetRecord};
use golden_core::model::NodeUuid;
use golden_video::Extent2D;
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub const MEDIA_NAMESPACE: &str = "mapgyver.media";
pub const MATRIX_PLAYBACK_EVENT: &str = "mapgyver.matrix.playback";
pub const APPLICATION_RUNTIME_EVENT: &str = "mapgyver-applications-runtime";
pub const MEDIA_OPERATION: &str = "edit";
pub const ASSET_NODE_TYPE: &str = "mapgyver.media.asset.v2";
pub const SENDER_DISCOVERY_EVENT: &str = "mapgyver-sender-discovery";

pub use super::commands::{MediaCommandCatalogItem, MediaCommandKind};
pub use super::target_picker::protocol::*;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MediaNativeRequest {
    #[serde(rename = "mapgyverSenders")]
    Senders { refresh: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
pub struct SenderDiscoverySnapshot {
    pub revision: u32,
    pub refreshing: bool,
    pub names: Vec<String>,
    pub truncated: bool,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum SenderSelectionMode {
    #[default]
    Manual,
    Auto,
}
impl SenderSelectionMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Manual" => Ok(Self::Manual),
            "Auto" => Ok(Self::Auto),
            _ => Err("sender selection must be Manual or Auto".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Auto => "Auto",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct MediaId(pub NodeUuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct ChannelId(pub NodeUuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct ChannelReference {
    pub media: MediaId,
    pub channel: ChannelId,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum ChannelMoveDirection {
    Earlier,
    Later,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum MediaKind {
    Reference,
    Picture,
    SolidColor,
    SharedTexture,
    Composition,
    Matrix,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MediaCreationChoice {
    pub media_kind: MediaKind,
    pub label: String,
}

impl MediaKind {
    pub(crate) fn default_label(self) -> &'static str {
        match self {
            Self::Reference => "Reference",
            Self::Picture => "Picture",
            Self::SolidColor => "Solid Color",
            Self::SharedTexture => "Shared Texture",
            Self::Composition => "Composition",
            Self::Matrix => "Matrix",
        }
    }

    pub(crate) fn has_authored_channel_variants(self) -> bool {
        matches!(self, Self::Picture | Self::SolidColor | Self::SharedTexture)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum ChannelChoiceScope {
    All,
    MatrixFinal,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ChannelChoiceQuery {
    pub scope: ChannelChoiceScope,
    pub search: String,
    pub start: u32,
    pub count: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ChannelChoice {
    pub reference: ChannelReference,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ChannelChoicePage {
    pub choices: Vec<ChannelChoice>,
    pub total: u32,
    pub start: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct MediaChannelsQuery {
    pub media: MediaId,
    pub start: u32,
    pub count: u16,
}

/// Authored channel membership, independent of runtime input availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum MediaChannelsStatus {
    Ready,
    Unconfigured,
    MissingTarget,
    MissingMedia,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct MediaChannelsPage {
    pub media: MediaId,
    pub status: MediaChannelsStatus,
    pub start: u32,
    pub total: u32,
    pub channels: Vec<ChannelSnapshot>,
}

/// Panel selection and paging hints; no authored settings are changed by inspection.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct MediaPreviewQuery {
    pub selected: Option<NodeUuid>,
    pub document: Option<NodeUuid>,
    pub channel: Option<ChannelId>,
    pub start: u32,
    pub count: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct MediaPreviewPage {
    pub document: NodeUuid,
    pub document_matches: bool,
    pub media: Option<MediaId>,
    pub label: Option<String>,
    pub channels: Option<MediaChannelsPage>,
    /// Independent bounded lookup keeps a chosen channel available outside the visible page.
    pub selected: Option<ChannelSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum SizePolicy {
    Automatic,
    Fixed { extent: Extent2D },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
#[serde(deny_unknown_fields)]
pub enum ChannelSource {
    Reference {
        target: Option<ChannelReference>,
    },
    Matrix {
        output: crate::matrix::protocol::MatrixOutput,
    },
    Composition {
        canvas: Extent2D,
    },
    Picture {
        file: Option<AssetLocator>,
    },
    /// Straight linear-light RGBA. Conversion/premultiplication happens in the backend.
    SolidColor {
        rgba: [f32; 4],
    },
    /// A missing selector is authored as unconfigured, never the global active sender.
    Spout {
        sender: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct ChannelDefinition {
    pub source: ChannelSource,
    /// Effective projection of the owning media Resolution, never channel-authored state.
    pub size: SizePolicy,
    /// Source placement projected from the owning media; ignored at intrinsic resolution.
    pub fit_mode: MediaFitMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum MediaFitMode {
    #[default]
    Fit,
    Fill,
    Stretch,
}

impl MediaFitMode {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Fit => "Fit",
            Self::Fill => "Fill",
            Self::Stretch => "Stretch",
        }
    }
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Fit" => Ok(Self::Fit),
            "Fill" => Ok(Self::Fill),
            "Stretch" => Ok(Self::Stretch),
            _ => Err("unknown media Fit Mode".into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MediaEdit {
    InspectTargetPicker {
        query: TargetPickerQuery,
    },
    SelectTargetPickerChoice {
        #[ts(type = "string")]
        parameter: NodeUuid,
        #[ts(type = "string | null")]
        target: Option<NodeUuid>,
    },
    CreateReference {
        target: Option<MediaId>,
        label: Option<String>,
    },
    ConfigureReference {
        media: MediaId,
        target: Option<MediaId>,
    },
    SelectReferenceChannel {
        media: MediaId,
        channel: Option<ChannelId>,
    },
    Applications {
        edit: crate::applications::protocol::ApplicationEdit,
    },
    InspectChoices {
        query: ChannelChoiceQuery,
    },
    InspectChannels {
        query: MediaChannelsQuery,
    },
    InspectMediaPreview {
        query: MediaPreviewQuery,
    },
    InspectCompositionEditor {
        query: super::composition::protocol::CompositionEditorQuery,
    },
    InspectMatrixEditor {
        query: crate::matrix::protocol::MatrixEditorQuery,
    },
    InspectMatrixCells {
        query: crate::matrix::protocol::MatrixCellLayoutQuery,
    },
    InspectSpatialEditor {
        query: super::spatial::protocol::SpatialEditorQuery,
    },
    Matrix {
        media: MediaId,
        edit: crate::matrix::protocol::MatrixEdit,
    },
    Composition {
        reference: ChannelReference,
        edit: super::composition::protocol::CompositionEdit,
    },
    InspectManager,
    InspectCommandCatalog,
    CreateCommand {
        command_kind: MediaCommandKind,
        label: Option<String>,
    },
    ExecuteCommand {
        #[ts(type = "string")]
        command: NodeUuid,
    },
    /// Creates an app-template source script under Medias, outside its object catalog.
    CreateScript {
        label: Option<String>,
    },
    /// Creates a self-contained Picture, or edits the target Picture's own File Parameter.
    ImportPicture {
        locator: AssetLocator,
        target: Option<ChannelReference>,
        label: Option<String>,
    },
    ImportAsset {
        locator: AssetLocator,
        label: Option<String>,
    },
    RelinkAsset {
        asset: AssetId,
        locator: AssetLocator,
    },
    /// Reprepare the current host file without changing its locator or authored history.
    ReloadAsset {
        asset: AssetId,
    },
    RemoveAsset {
        asset: AssetId,
    },
    InspectAsset {
        asset: AssetId,
    },
    CreateMedia {
        media_kind: MediaKind,
        label: Option<String>,
    },
    CreateFolder {
        #[ts(type = "string | null")]
        parent: Option<NodeUuid>,
        label: Option<String>,
    },
    CreateMediaInFolder {
        #[ts(type = "string")]
        folder: NodeUuid,
        media_kind: MediaKind,
        label: Option<String>,
    },
    RenameFolder {
        #[ts(type = "string")]
        folder: NodeUuid,
        label: String,
    },
    RemoveFolder {
        #[ts(type = "string")]
        folder: NodeUuid,
    },
    /// Move a folder or media entry. The backend validates containment and cycles.
    MoveEntry {
        #[ts(type = "string")]
        entry: NodeUuid,
        #[ts(type = "string | null")]
        parent: Option<NodeUuid>,
    },
    AddChannel {
        media: MediaId,
        label: Option<String>,
    },
    ConfigureChannel {
        reference: ChannelReference,
        source: ChannelSource,
    },
    ConfigurePictureFile {
        reference: ChannelReference,
        file: String,
    },
    /// Reprepare this Picture's File without changing authored state or history.
    ReloadPicture {
        reference: ChannelReference,
    },
    InspectPictureFile {
        reference: ChannelReference,
    },
    /// Shared output sizing; channel definitions project this setting and cannot change it.
    ConfigureMediaResolution {
        media: MediaId,
        size: SizePolicy,
    },
    ConfigureMediaFitMode {
        media: MediaId,
        mode: MediaFitMode,
    },
    ConfigureMediaSenderSelection {
        media: MediaId,
        mode: SenderSelectionMode,
    },
    RenameMedia {
        media: MediaId,
        label: String,
    },
    RenameChannel {
        reference: ChannelReference,
        label: String,
    },
    ReorderChannel {
        reference: ChannelReference,
        after: Option<ChannelId>,
    },
    MoveChannel {
        reference: ChannelReference,
        direction: ChannelMoveDirection,
    },
    RemoveChannel {
        reference: ChannelReference,
    },
    RemoveMedia {
        media: MediaId,
    },
    InspectChannel {
        reference: ChannelReference,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ChannelSnapshot {
    pub reference: ChannelReference,
    pub label: String,
    pub definition: ChannelDefinition,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct MediaSnapshot {
    pub id: MediaId,
    pub label: String,
    pub media_kind: MediaKind,
    pub sender_selection: Option<SenderSelectionMode>,
    pub channels: Vec<ChannelSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct MediaManagerSnapshot {
    pub media: Vec<MediaSnapshot>,
    pub assets: Vec<AssetSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct AssetSnapshot {
    pub id: AssetId,
    pub label: String,
    pub locator: Option<AssetLocator>,
}

/// Exact process-local address of the actual typed File; never stored in a project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct FileParameterAddress {
    pub node: golden_core::node::NodeId,
    #[ts(type = "string")]
    pub uuid: NodeUuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PictureFileBinding {
    pub reference: ChannelReference,
    pub parameter: FileParameterAddress,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MediaEditResult {
    TargetPicker {
        page: Option<TargetPickerPage>,
    },
    PictureFile {
        binding: Option<PictureFileBinding>,
    },
    MatrixCellPrepared {
        cell: crate::matrix::protocol::CellId,
        application: crate::applications::protocol::ApplicationId,
    },
    MatrixColumnGroupCreated {
        group: crate::matrix::protocol::ColumnGroupId,
    },
    CompositionEditor {
        page: super::composition::protocol::CompositionEditorPage,
    },
    MatrixEditor {
        page: crate::matrix::protocol::MatrixEditorPage,
    },
    MatrixCellLayout {
        page: crate::matrix::protocol::MatrixCellLayoutPage,
    },
    SpatialEditor {
        binding: super::spatial::protocol::SpatialEditorBinding,
    },
    MatrixLayerGeometryChanged {
        media: MediaId,
        layer: crate::matrix::protocol::LayerId,
        settings: crate::matrix::protocol::LayerSettings,
    },
    MatrixCellGeometryChanged {
        media: MediaId,
        cell: crate::matrix::protocol::CellId,
        settings: crate::matrix::protocol::CellSettings,
    },
    MediaPreview {
        page: MediaPreviewPage,
    },
    MatrixMediaCreated {
        cell: crate::matrix::protocol::CellId,
        media: MediaId,
    },
    Channels {
        page: MediaChannelsPage,
    },
    ReferenceCreated {
        media: MediaId,
    },
    CommandCatalog {
        commands: Vec<MediaCommandCatalogItem>,
    },
    CommandCreated {
        #[ts(type = "string")]
        command: NodeUuid,
    },
    ScriptCreated {
        #[ts(type = "string")]
        script: NodeUuid,
    },
    FolderCreated {
        #[ts(type = "string")]
        folder: NodeUuid,
    },
    ApplicationCallbacks {
        state: crate::applications::protocol::ApplicationCallbackState,
    },
    ApplicationRuntime {
        snapshot: crate::applications::protocol::ApplicationRuntimeSnapshot,
    },
    Application {
        snapshot: crate::applications::protocol::ApplicationSnapshot,
    },
    /// A query can arrive after deletion even when its caller's selection was valid.
    ApplicationMissing {
        application: crate::applications::protocol::ApplicationId,
    },
    MatrixTargets {
        page: crate::matrix::protocol::MatrixTargetPage,
    },
    Choices {
        page: ChannelChoicePage,
    },
    MatrixViewport {
        snapshot: crate::matrix::protocol::MatrixViewportSnapshot,
    },
    /// A bounded viewport read may arrive after its Matrix was removed.
    MatrixMissing {
        media: MediaId,
    },
    MatrixControlInvoked {
        owner: NodeUuid,
        parameter: NodeUuid,
    },
    MatrixPlayback {
        snapshot: crate::matrix::protocol::MatrixPlaybackSnapshot,
    },
    MatrixCreated {
        items: Vec<crate::matrix::protocol::MatrixItemId>,
        columns: Vec<crate::matrix::protocol::ColumnId>,
    },
    Matrix {
        snapshot: crate::matrix::protocol::MatrixSnapshot,
    },
    InstanceChanged {
        snapshot: super::composition::protocol::InstanceSnapshot,
    },
    Composition {
        snapshot: super::composition::protocol::CompositionSnapshot,
    },
    InstancesCreated {
        instances: Vec<super::composition::protocol::InstanceReference>,
    },
    Manager {
        snapshot: MediaManagerSnapshot,
    },
    PictureImported {
        reference: ChannelReference,
    },
    AssetCreated {
        asset: AssetId,
    },
    Asset {
        record: AssetRecord,
    },
    Created {
        reference: ChannelReference,
    },
    Changed,
    Channel {
        snapshot: Option<ChannelSnapshot>,
    },
}

pub fn export_typescript(directory: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    let directory = directory.as_ref();
    if directory.is_dir() {
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_file() && path.extension().is_some_and(|extension| extension == "ts") {
                let text = std::fs::read_to_string(&path)?;
                if text.starts_with("// This file was generated by [ts-rs]")
                    || text.starts_with("// Generated by generate_media_protocol;")
                {
                    std::fs::remove_file(path)?;
                }
            }
        }
    }
    let config = Config::default().with_out_dir(directory).with_large_int("number");
    MediaEdit::export_all(&config)?;
    MediaEditResult::export_all(&config)?;
    let fields = super::commands::MediaCommandKind::ALL
        .iter()
        .map(|kind| (kind.key(), kind.fields()))
        .collect::<std::collections::BTreeMap<_, _>>();
    std::fs::write(
        directory.join("command-fields.ts"),
        format!(
            "// Generated by generate_media_protocol; do not edit.\nexport const COMMAND_PARAMETER_FIELDS = {} as const;\n",
            serde_json::to_string_pretty(&fields)?
        ),
    )?;
    crate::applications::protocol::ApplicationLifecycleEvent::export_all(&config)?;
    MediaNativeRequest::export_all(&config)?;
    SenderDiscoverySnapshot::export_all(&config)?;
    super::resolution::ChannelResolution::export_all(&config)?;
    std::fs::write(
        directory.join("constants.ts"),
        format!(
            "// Generated by generate_media_protocol; do not edit.\nexport const MEDIA_NAMESPACE = {MEDIA_NAMESPACE:?};\nexport const MEDIA_OPERATION = {MEDIA_OPERATION:?};\nexport const ASSET_NODE_TYPE = {ASSET_NODE_TYPE:?};\nexport const SENDER_DISCOVERY_EVENT = {SENDER_DISCOVERY_EVENT:?};\nexport const MATRIX_PLAYBACK_EVENT = {MATRIX_PLAYBACK_EVENT:?};\nexport const APPLICATION_RUNTIME_EVENT = {APPLICATION_RUNTIME_EVENT:?};\n"
        ),
    )?;
    // ts-rs leaves trailing spaces around embedded field documentation.
    // Normalize the owned generated directory so drift and whitespace checks agree.
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "ts") {
            let text = std::fs::read_to_string(&path)?;
            let normalized = text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
            std::fs::write(path, format!("{normalized}\n"))?;
        }
    }
    Ok(())
}
