//! Rust-owned app intents and authored contracts, also used by project codecs.

pub use golden_assets::{AssetId, AssetLocator, AssetRecord};
use golden_core::model::NodeUuid;
use golden_video::plan::SampleFilter;
use golden_video::{Extent2D, TextureDescriptor};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub const MEDIA_NAMESPACE: &str = "mapgyver.media";
pub const MATRIX_PLAYBACK_EVENT: &str = "mapgyver.matrix.playback";
pub const APPLICATION_RUNTIME_EVENT: &str = "mapgyver-applications-runtime";
pub const MEDIA_OPERATION: &str = "edit";
pub const SENDER_DISCOVERY_EVENT: &str = "mapgyver-sender-discovery";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct MediaId(pub NodeUuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct ChannelId(pub NodeUuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct ChannelReference {
    pub media: MediaId,
    pub channel: ChannelId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum MediaKind {
    Picture,
    SolidColor,
    SharedTexture,
    Composition,
    Matrix,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum SizePolicy {
    Automatic,
    Fixed { extent: Extent2D },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ChannelSource {
    Matrix {
        output: crate::matrix::protocol::MatrixOutput,
    },
    Composition {
        canvas: Extent2D,
    },
    Picture {
        asset: Option<AssetId>,
    },
    /// Straight linear-light RGBA. Conversion/premultiplication happens in the backend.
    SolidColor {
        rgba: [f32; 4],
    },
    /// A missing selector is authored as unconfigured, never the global active sender.
    Spout {
        sender: Option<String>,
        maximum: TextureDescriptor,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct ChannelDefinition {
    pub source: ChannelSource,
    pub size: SizePolicy,
    /// Full-image stretch is the initial fit policy; filter choice is explicit.
    pub filter: SampleFilter,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MediaEdit {
    Applications {
        edit: crate::applications::protocol::ApplicationEdit,
    },
    InspectChoices {
        query: ChannelChoiceQuery,
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
    /// Atomically imports an asset and creates Picture media, or assigns an existing Picture channel.
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
    AddChannel {
        media: MediaId,
        label: Option<String>,
    },
    ConfigureChannel {
        reference: ChannelReference,
        definition: ChannelDefinition,
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

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MediaEditResult {
    ApplicationCallbacks {
        state: crate::applications::protocol::ApplicationCallbackState,
    },
    ApplicationReferences {
        references: Vec<crate::applications::protocol::ApplicationReference>,
    },
    ApplicationRuntime {
        snapshot: crate::applications::protocol::ApplicationRuntimeSnapshot,
    },
    ApplicationCreated {
        application: crate::applications::protocol::ApplicationId,
    },
    Application {
        snapshot: crate::applications::protocol::ApplicationSnapshot,
    },
    Applications {
        page: crate::applications::protocol::ApplicationPage,
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
    MatrixLaunched {
        snapshot: crate::matrix::protocol::MatrixPlaybackSnapshot,
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
        asset: AssetId,
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
        snapshot: ChannelSnapshot,
    },
}

pub fn export_typescript(directory: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    let directory = directory.as_ref();
    let config = Config::default().with_out_dir(directory).with_large_int("number");
    MediaEdit::export_all(&config)?;
    MediaEditResult::export_all(&config)?;
    crate::applications::protocol::ApplicationLifecycleEvent::export_all(&config)?;
    MediaNativeRequest::export_all(&config)?;
    SenderDiscoverySnapshot::export_all(&config)?;
    super::resolution::ChannelResolution::export_all(&config)?;
    std::fs::write(
        directory.join("constants.ts"),
        format!(
            "// Generated by generate_media_protocol; do not edit.\nexport const MEDIA_NAMESPACE = {MEDIA_NAMESPACE:?};\nexport const MEDIA_OPERATION = {MEDIA_OPERATION:?};\nexport const SENDER_DISCOVERY_EVENT = {SENDER_DISCOVERY_EVENT:?};\nexport const MATRIX_PLAYBACK_EVENT = {MATRIX_PLAYBACK_EVENT:?};\nexport const APPLICATION_RUNTIME_EVENT = {APPLICATION_RUNTIME_EVENT:?};\n"
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
