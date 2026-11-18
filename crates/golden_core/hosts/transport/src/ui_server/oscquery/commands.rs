use super::{Settings, feedback::Feedback, graph, preferences};
use crate::ui_server::{ServerState, save_preferences_after_change};
use golden_engine::{app::ProjectLifecycle, parameter::Color, ui_read_model::UiSnapshotCapture};
use golden_protocol::{NodeMetaPatch, UiEditIntent, UiNodeDto, UiSubscriptionScope};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Request {
    #[serde(rename = "COMMAND")]
    command: String,
    #[serde(rename = "DATA", default)]
    data: Value,
    #[serde(rename = "ID")]
    id: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SubscriptionData {
    Address(String),
    AddressOnly((String,)),
    AddressAndFeedback((String, bool)),
    Options {
        address: String,
        #[serde(rename = "sendFeedback", default)]
        send_feedback: bool,
    },
}

pub(super) struct Reply {
    pub(super) data: Value,
    pub(super) initial_value: Option<Vec<u8>>,
}

#[derive(Deserialize)]
struct Address {
    address: String,
}

#[derive(Deserialize)]
struct Add {
    address: String,
    #[serde(rename = "type")]
    node_type: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct Rename {
    address: String,
    name: String,
}

#[derive(Deserialize)]
struct Move {
    address: String,
    parent: String,
    before: Option<String>,
}

#[derive(Deserialize)]
struct Patch {
    address: String,
    meta: EditableMetadata,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EditableMetadata {
    label: Option<String>,
    enabled: Option<bool>,
    #[serde(default, deserialize_with = "nullable")]
    description: Option<Option<String>>,
    tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "nullable")]
    color: Option<Option<Color>>,
    collapsed: Option<bool>,
}

fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

struct Prepared<'a> {
    intent: UiEditIntent,
    target: Option<&'a UiNodeDto>,
    add_address: Option<String>,
}

pub(super) fn handle<T: ProjectLifecycle>(
    state: &ServerState<T>,
    settings: Settings,
    feedback: &mut Feedback,
    text: &str,
) -> Reply {
    let request = match serde_json::from_str::<Request>(text) {
        Ok(request) => request,
        Err(_) => {
            return Reply {
                data: json!({ "COMMAND": "RESULT", "DATA": { "ok": false, "error": "Invalid OSCQuery command" } }),
                initial_value: None,
            };
        }
    };
    let mut initial_value = None;
    let result = if matches!(request.command.as_str(), "LISTEN" | "IGNORE") {
        subscription(state, settings, feedback, &request).map(|initial| {
            initial_value = initial;
            None
        })
    } else {
        apply(state, settings, &request)
    };
    let mut response = json!({ "COMMAND": "RESULT", "DATA": { "command": request.command, "ok": result.is_ok() } });
    if let Some(id) = request.id {
        response["ID"] = json!(id);
    }
    match result {
        Ok(Some(address)) => response["DATA"]["address"] = json!(address),
        Ok(None) => {}
        Err(error) => response["DATA"]["error"] = json!(error),
    }
    Reply {
        data: response,
        initial_value,
    }
}

fn subscription<T: ProjectLifecycle>(
    state: &ServerState<T>,
    settings: Settings,
    feedback: &mut Feedback,
    request: &Request,
) -> Result<Option<Vec<u8>>, String> {
    let data = payload(&request.data)?;
    let (address, send_feedback) = match data {
        SubscriptionData::Address(address) | SubscriptionData::AddressOnly((address,)) => (address, false),
        SubscriptionData::AddressAndFeedback((address, send_feedback))
        | SubscriptionData::Options { address, send_feedback } => (address, send_feedback),
    };
    if request.command == "IGNORE" {
        feedback.ignore(&address);
        return Ok(None);
    }
    let snapshot = state.read_model.capture_snapshot(UiSubscriptionScope::WholeGraph);
    if preferences::enabled_settings(&snapshot) != Some(settings)
        || snapshot.project_generation().get() != feedback.project_generation()
    {
        return Err("OSCQuery listener or project changed".into());
    }
    feedback
        .subscribe(&snapshot, address, send_feedback)
        .map_err(str::to_owned)
}

fn apply<T: ProjectLifecycle>(
    state: &ServerState<T>,
    settings: Settings,
    request: &Request,
) -> Result<Option<String>, String> {
    let snapshot = state.read_model.capture_snapshot(UiSubscriptionScope::WholeGraph);
    if preferences::enabled_settings(&snapshot) != Some(settings) {
        return Err("OSCQuery listener changed".into());
    }
    let project = snapshot.project_generation();
    let prepared = prepare(&snapshot, request)?;
    let expected_meta = prepared.target.map(|target| (target.node_id, target.meta.clone()));
    let result = state
        .runtime
        .apply_ui_transaction_batch_for_project(project, vec![prepared.intent], expected_meta)
        .map_err(|error| error.to_string())?;
    save_preferences_after_change(&state.runtime, state.preferences.as_ref(), result.preferences_changed);
    let ack = result
        .transactions
        .into_iter()
        .next()
        .expect("one intent returns one acknowledgement")
        .acknowledgement;
    if !ack.success {
        return Err(ack.error_message.unwrap_or_else(|| "Golden rejected the edit".into()));
    }
    let Some((address, created)) = prepared.add_address.zip(ack.created_node) else {
        return Ok(None);
    };
    let after = state.read_model.capture_snapshot(UiSubscriptionScope::WholeGraph);
    if after.project_generation() != project {
        return Ok(None);
    }
    let parent = graph::resolve(&after, &address);
    Ok(parent.and_then(|parent| graph::child_path(&after, parent, &address, &created)))
}

fn prepare<'a>(snapshot: &'a UiSnapshotCapture, request: &Request) -> Result<Prepared<'a>, String> {
    let find = |address: &str| graph::resolve(snapshot, address).ok_or_else(|| "Unknown OSC address".to_string());
    let mut added = None;
    let (intent, target) = match request.command.as_str() {
        "ADD" => {
            let Add {
                address,
                node_type,
                name,
            } = payload(&request.data)?;
            let parent = find(&address)?;
            let item = parent
                .creatable_user_items
                .iter()
                .find(|item| item.label == node_type)
                .or_else(|| {
                    let mut matching = parent
                        .creatable_user_items
                        .iter()
                        .filter(|item| item.node_type == node_type);
                    let item = matching.next()?;
                    matching.next().is_none().then_some(item)
                })
                .ok_or("Choose an item from GOLDEN_ADD_TYPES for this manager")?;
            let label = name.map(normalize_label).transpose()?;
            added = Some(address);
            (
                UiEditIntent::CreateUserItem {
                    parent: parent.node_id,
                    node_type: item.node_type.clone(),
                    label: Some(label.unwrap_or_else(|| item.label.clone())),
                    initial_params: item.initial_params.clone(),
                },
                Some(parent),
            )
        }
        "REMOVE" => {
            let Address { address } = payload(&request.data)?;
            let node = find(&address)?;
            require(
                node.meta.user_permissions.can_remove_and_duplicate,
                "This node cannot be removed",
            )?;
            (UiEditIntent::RemoveNode { node: node.node_id }, Some(node))
        }
        "RENAME" => {
            let Rename { address, name } = payload(&request.data)?;
            let node = find(&address)?;
            let patch = metadata(
                node,
                EditableMetadata {
                    label: Some(name),
                    ..Default::default()
                },
            )?;
            (
                UiEditIntent::PatchMeta {
                    node: node.node_id,
                    patch,
                },
                Some(node),
            )
        }
        "MOVE" => {
            let Move {
                address,
                parent,
                before,
            } = payload(&request.data)?;
            let node = find(&address)?;
            let parent = find(&parent)?;
            require(
                node.meta.user_permissions.can_remove_and_duplicate,
                "This node cannot be moved",
            )?;
            let before = before
                .as_deref()
                .filter(|address| !address.is_empty())
                .map(find)
                .transpose()?;
            if let Some(before) = before {
                require(
                    before.node_id != node.node_id && parent.children.contains(&before.node_id),
                    "The insertion target must be another direct child of the destination",
                )?;
            }
            let new_prev_sibling = parent
                .children
                .iter()
                .copied()
                .take_while(|id| before.is_none_or(|before| *id != before.node_id))
                .filter(|id| *id != node.node_id)
                .last();
            (
                UiEditIntent::MoveNode {
                    node: node.node_id,
                    new_parent: parent.node_id,
                    new_prev_sibling,
                },
                Some(node),
            )
        }
        "PATCH_META" => {
            let Patch { address, meta } = payload(&request.data)?;
            let node = find(&address)?;
            (
                UiEditIntent::PatchMeta {
                    node: node.node_id,
                    patch: metadata(node, meta)?,
                },
                Some(node),
            )
        }
        "UNDO" => (UiEditIntent::Undo, None),
        "REDO" => (UiEditIntent::Redo, None),
        _ => return Err("Unknown OSCQuery command".into()),
    };
    Ok(Prepared {
        intent,
        target,
        add_address: added,
    })
}

fn payload<T: DeserializeOwned>(data: &Value) -> Result<T, String> {
    T::deserialize(data).map_err(|error| format!("Invalid command data: {error}"))
}

fn require(allowed: bool, error: &str) -> Result<(), String> {
    if allowed { Ok(()) } else { Err(error.into()) }
}

fn normalize_label(label: String) -> Result<String, String> {
    let label = label.trim();
    require(!label.is_empty(), "A node name must not be empty")?;
    Ok(label.into())
}

fn metadata(node: &UiNodeDto, meta: EditableMetadata) -> Result<NodeMetaPatch, String> {
    let permissions = &node.meta.user_permissions;
    require(
        meta.label.is_none() && meta.description.is_none() || permissions.can_edit_name,
        "This node's name and description are locked",
    )?;
    require(
        meta.enabled.is_none() || node.meta.can_be_disabled,
        "This node cannot be disabled",
    )?;
    require(
        meta.tags.is_none() || permissions.can_edit_tags,
        "This node's tags are locked",
    )?;
    require(
        meta.color.is_none() || permissions.can_edit_color,
        "This node's color is locked",
    )?;
    let presentation = if meta.color.is_some() || meta.collapsed.is_some() {
        let mut presentation = node.meta.presentation.clone();
        if let Some(color) = meta.color {
            if let Some(color) = color {
                require(
                    [color.red, color.green, color.blue, color.alpha]
                        .into_iter()
                        .all(|value| (0.0..=1.0).contains(&value)),
                    "Color components must be between 0 and 1",
                )?;
            }
            presentation.color = color;
        }
        if let Some(collapsed) = meta.collapsed {
            presentation.collapsed = collapsed;
        }
        Some(presentation)
    } else {
        None
    };
    Ok(NodeMetaPatch {
        label: meta.label.map(normalize_label).transpose()?,
        enabled: meta.enabled,
        description: meta.description,
        tags: meta.tags,
        presentation,
        ..Default::default()
    })
}
