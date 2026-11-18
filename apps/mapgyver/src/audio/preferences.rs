use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::{Edit, NodeTree},
    engine::{NodeExecutionRule, NodeWaker},
    node,
    node::{
        DeclId, Node, NodeCreationContext, NodeId, NodeUuid, ParameterHandle, UserContainerRules, UserCreatableItem,
    },
    parameter::{
        Enum, ParamValue, ParameterEnumDomainPolicy, ParameterEnumOption, ParameterEnumOptions, ParameterEventBehaviour,
    },
    process_ctx::ProcessCtx,
};
use std::sync::{Arc, Mutex};

const OUTPUT_TYPE: &str = "mapgyver.preferences.audio.output";
const DECL_ID: &str = "mapgyver_audio";
pub(super) const DEFAULT: &str = "System Default";
pub(super) const UNASSIGNED: &str = "Unassigned";

#[node(
    "mapgyver.preferences.audio",
    label = "Audio",
    can_be_disabled = false,
    collapsed = false
)]
#[children(
    output_enabled: bool = false (label = "Enable output");
    device: Enum = "System Default" (label = "Output device",
        enum_domain = ParameterEnumDomainPolicy::Open,
        enum_options = crate::parameters::enum_options(&[DEFAULT]));
)]
pub struct AudioPreferencesNode {
    #[state(default = Default::default())]
    inbox: Arc<Mutex<Option<Publication>>>,
    #[state(default = None)]
    waker: Option<NodeWaker>,
    #[state(default = Default::default())]
    status: Status,
}

#[node("mapgyver.preferences.audio", from_struct)]
impl Node for AudioPreferencesNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive()
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: NodeCreationContext) {
        self.waker = ctx.node_waker().ok();
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        let Some(update) = self.inbox.lock().expect("Audio Preferences inbox").take() else {
            return;
        };
        self.status = update.status;
        for edit in update.choices {
            ctx.edits.push(Edit::SetParamEnumOptions {
                node: edit.node,
                expected_uuid: edit.uuid,
                expected_value: ParamValue::Enum(edit.selected),
                selected_value: None,
                options: edit.options,
            });
        }
        if let Some(error) = &self.status.error {
            ctx.set_node_warning_with(self.id(), Some("audio-output"), error.clone(), None);
        } else {
            ctx.clear_node_warning(self.id(), Some("audio-output"));
        }
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[OUTPUT_TYPE]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(OUTPUT_TYPE, OUTPUT_TYPE, "Virtual Output")]
    }
    fn create_user_item_tree(&self, kind: &str) -> Option<NodeTree> {
        if kind != OUTPUT_TYPE {
            return None;
        }
        let mut output = OutputNode::new();
        output.channel_key = ParameterHandle::new(uuid::Uuid::new_v4().to_string());
        Some(NodeTree::new(output))
    }
}

#[node("mapgyver.preferences.audio.output", label = "Virtual Output", can_be_disabled = false, collapsed = false,
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
    })]
#[children(
    channel_key: String = String::new() (label = "Channel identity", read_only = true,
        persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
    physical: Enum = "Unassigned" (label = "Physical channel", enum_domain = ParameterEnumDomainPolicy::Open,
        enum_options = crate::parameters::enum_options(&[UNASSIGNED]));
)]
pub struct OutputNode {}

#[node("mapgyver.preferences.audio.output", from_struct)]
impl Node for OutputNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive()
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, creation: NodeCreationContext) {
        if matches!(
            creation,
            NodeCreationContext::Duplicate | NodeCreationContext::DuplicateWithInitialParams
        ) {
            ctx.edits.push(Edit::SetParam {
                node: self.channel_key.id(),
                value: ParamValue::Str(uuid::Uuid::new_v4().to_string()),
                behaviour: ParameterEventBehaviour::Append,
            });
        }
    }
}

pub(crate) fn tree() -> NodeTree {
    let mut node = AudioPreferencesNode::new();
    node.node_data_mut().meta.decl_id = DeclId(DECL_ID.into());
    let mut tree = NodeTree::new(node);
    for (key, label, physical, channel) in [
        ("main-left", "Left", "output:0", super::voice::LEFT),
        ("main-right", "Right", "output:1", super::voice::RIGHT),
    ] {
        let mut output = OutputNode::new();
        let meta = &mut output.node_data_mut().meta;
        meta.label = label.into();
        meta.decl_id = DeclId(key.into());
        meta.user_permissions.can_remove_and_duplicate = false;
        output.channel_key = ParameterHandle::new(channel.to_string());
        output.physical = ParameterHandle::new(physical.into());
        tree.push_child(NodeTree::new(output));
    }
    tree
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub(super) struct Status {
    pub enabled: bool,
    pub selected: String,
    pub readiness: Option<golden_audio::AudioDeviceReadiness>,
    pub active_device: Option<golden_audio::AudioDeviceTargetId>,
    pub format: Option<golden_audio::NegotiatedStreamFormat>,
    pub physical_channels: Vec<String>,
    pub routes: Vec<(String, String)>,
    pub output_levels: Vec<golden_audio::ChannelObservation>,
    pub configuration_generation: u64,
    pub error: Option<String>,
}

pub(super) struct Publication {
    pub status: Status,
    pub choices: Vec<ChoiceEdit>,
}

pub(super) struct ChoiceEdit {
    node: NodeId,
    uuid: NodeUuid,
    selected: String,
    options: ParameterEnumOptions,
}

#[derive(Clone)]
pub(super) struct Choice {
    node: NodeId,
    uuid: NodeUuid,
    pub value: String,
    options: ParameterEnumOptions,
}
impl Choice {
    fn read(engine: &AppEngine, id: NodeId) -> Result<Self, String> {
        let node = engine.nodes.get(id).ok_or("Audio option is missing")?;
        let param = node.engine_param_snapshot().ok_or("Audio option is not a Parameter")?;
        let ParamValue::Enum(value) = param.value else {
            return Err("Audio option is not an Enum".into());
        };
        Ok(Self {
            node: id,
            uuid: node.node_data().meta.uuid,
            value,
            options: param.constraints.enum_options,
        })
    }

    pub fn update(&self, available: &ParameterEnumOptions) -> Option<ChoiceEdit> {
        // Open choices retain their exact authored key even while it is unavailable.
        let options = if available.contains_variant(&self.value) {
            available.clone()
        } else {
            let label = self
                .options
                .as_slice()
                .iter()
                .find(|entry| entry.variant_id == self.value)
                .map_or(self.value.as_str(), |entry| entry.label.as_str());
            let label = label.strip_suffix(" (unavailable)").unwrap_or(label);
            let mut options = available.as_slice().to_vec();
            options.push(option(&self.value, format!("{label} (unavailable)")));
            options.into()
        };
        (self.options != options).then(|| ChoiceEdit {
            node: self.node,
            uuid: self.uuid,
            selected: self.value.clone(),
            options,
        })
    }
}

pub(super) fn option(id: &str, label: String) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: id.into(),
        value: ParamValue::Enum(id.into()),
        label,
        tags: vec![],
        ordering: None,
    }
}

pub(super) struct VirtualOutput {
    pub key: String,
    pub label: String,
    pub physical: Choice,
}

pub(super) struct Settings {
    pub enabled: bool,
    pub device: Choice,
    pub outputs: Vec<VirtualOutput>,
    pub published: Status,
    inbox: Arc<Mutex<Option<Publication>>>,
    waker: NodeWaker,
}
impl Settings {
    pub fn capture(engine: &AppEngine) -> Result<Self, String> {
        let preferences = find(engine).ok_or("Audio Preferences are missing")?;
        let mut outputs = Vec::new();
        let mut child = preferences.node_data().first_child;
        while let Some(id) = child {
            let node = engine.nodes.get(id).ok_or("Audio Preferences child is missing")?;
            child = node.node_data().next_sibling;
            if let AppNode::AudioOutput(output) = node {
                outputs.push(VirtualOutput {
                    key: output.channel_key.get_ref().clone(),
                    label: output.node_data().meta.label.clone(),
                    physical: Choice::read(engine, output.physical.id())?,
                });
                if outputs.len() > usize::from(golden_audio::EngineLimits::default().max_virtual_outputs) {
                    return Err("Too many virtual audio outputs".into());
                }
            }
        }
        Ok(Self {
            enabled: preferences.output_enabled.get(),
            device: Choice::read(engine, preferences.device.id())?,
            outputs,
            published: preferences.status.clone(),
            inbox: preferences.inbox.clone(),
            waker: preferences.waker.clone().ok_or("Audio Preferences are not active")?,
        })
    }

    pub fn publish(&self, update: Publication) {
        if !self.waker.is_closed() {
            *self.inbox.lock().expect("Audio Preferences inbox") = Some(update);
            let _ = self.waker.wake();
        }
    }
}

fn find(engine: &AppEngine) -> Option<&AudioPreferencesNode> {
    let root = golden_core::app::live_preferences_root(engine)?;
    let mut child = engine.nodes.get(root)?.node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id)?;
        if let AppNode::AudioPreferences(preferences) = node {
            return Some(preferences);
        }
        child = node.node_data().next_sibling;
    }
    None
}

pub(crate) fn inspect(engine: &AppEngine) -> Result<serde_json::Value, String> {
    let preferences = find(engine).ok_or("Audio Preferences are missing")?;
    serde_json::to_value(&preferences.status).map_err(|error| error.to_string())
}
