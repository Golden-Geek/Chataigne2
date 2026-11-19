//! Live availability metadata; authored choices and recovery remain independent.
use super::protocol::MediaKind;
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    engine::NodeWaker,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEnumOption, ParameterEnumOptions},
    process_ctx::ProcessCtx,
};
use golden_io::RevisionGuard;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex, Weak},
};

#[derive(Default)]
struct State {
    waker: Option<NodeWaker>,
    pending: Option<Vec<SelectionUpdate>>,
    closed: bool,
    catalog: Snapshot,
}
#[derive(Default)]
pub(crate) struct Inbox(Arc<Mutex<State>>);
impl Inbox {
    pub(super) fn snapshot(&self) -> Snapshot {
        self.0.lock().map(|state| state.catalog.clone()).unwrap_or_default()
    }
    pub fn register(&self, ctx: &ProcessCtx) -> Result<(), String> {
        self.0.lock().map_err(|_| "sender catalog inbox poisoned")?.waker =
            Some(ctx.node_waker().map_err(|e| e.to_string())?);
        Ok(())
    }
    pub fn apply(&self, ctx: &mut ProcessCtx) {
        let pending = self.0.lock().ok().and_then(|mut state| state.pending.take());
        for update in pending.into_iter().flatten() {
            if update.guard.is_current() {
                ctx.edits.push(Edit::SetParamEnumOptions {
                    node: update.node,
                    expected_uuid: update.uuid,
                    expected_value: update.value,
                    selected_value: None,
                    options: update.options,
                });
                if update.kind == MediaKind::Ndi {
                    if let Some(message) = update.warning.as_deref() {
                        ctx.set_node_warning_with(update.channel, Some("ndi-discovery"), message, None);
                    } else {
                        ctx.clear_node_warning(update.channel, Some("ndi-discovery"));
                    }
                }
            }
        }
    }
    fn publisher(&self) -> Result<Publisher, String> {
        let state = self.0.lock().map_err(|_| "sender catalog inbox poisoned")?;
        Ok(Publisher {
            state: Arc::downgrade(&self.0),
            waker: state.waker.clone().ok_or("sender catalog receiver not ready")?,
        })
    }
}
impl Drop for Inbox {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock() {
            state.closed = true;
            state.pending.take();
        }
    }
}
struct Publisher {
    state: Weak<Mutex<State>>,
    waker: NodeWaker,
}
impl Publisher {
    fn publish(&self, updates: Vec<SelectionUpdate>, catalog: Snapshot) -> Result<(), String> {
        let state = self.state.upgrade().ok_or("sender catalog retired")?;
        let mut state = state.lock().map_err(|_| "sender catalog inbox poisoned")?;
        if state.closed || self.waker.is_closed() {
            return Err("sender catalog retired".into());
        }
        // The first creation must see discovery even when no channel exists to receive metadata.
        state.catalog = catalog;
        if updates.is_empty() {
            return Ok(());
        }
        state.pending = Some(updates);
        drop(state);
        self.waker.wake().map_err(|e| e.to_string())
    }
}
struct Selection {
    channel: NodeId,
    node: NodeId,
    uuid: NodeUuid,
    value: String,
    options: ParameterEnumOptions,
    guard: RevisionGuard,
}
struct SelectionUpdate {
    kind: MediaKind,
    channel: NodeId,
    node: NodeId,
    uuid: NodeUuid,
    value: ParamValue,
    options: ParameterEnumOptions,
    guard: RevisionGuard,
    warning: Option<Arc<str>>,
}
pub(crate) struct Source {
    kind: MediaKind,
    publisher: Publisher,
    membership: (uuid::Uuid, u64),
    selections: Vec<Selection>,
    warning: Option<Arc<str>>,
    processed_warning: bool,
}
impl Source {
    fn dirty(&self) -> bool {
        self.publisher.waker.is_closed() || self.selections.iter().any(|s| !s.guard.is_current())
    }
    fn selection(engine: &AppEngine, id: NodeId) -> Result<Selection, String> {
        let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
            return Err("sender channel missing".into());
        };
        let node = channel.sender_parameter(super::protocol::SenderSelectionMode::Auto);
        let leaf = engine.nodes.get(node).ok_or("sender Enum missing")?;
        let parameter = leaf.engine_param_snapshot().ok_or("sender is not a Parameter")?;
        let ParamValue::Enum(value) = parameter.value else {
            return Err("sender is not an Enum".into());
        };
        Ok(Selection {
            channel: id,
            node,
            uuid: leaf.node_data().meta.uuid,
            value,
            options: parameter.constraints.enum_options,
            guard: channel.guard(),
        })
    }
    pub(crate) fn capture(engine: &AppEngine) -> Result<Self, String> {
        Self::capture_for(engine, MediaKind::SharedTexture)
    }

    pub(crate) fn capture_for(engine: &AppEngine, kind: MediaKind) -> Result<Self, String> {
        let manager = super::authoring::manager(engine)?;
        let Some(AppNode::MediaManager(manager)) = engine.nodes.get(manager) else {
            unreachable!()
        };
        let mut selections = Vec::new();
        for (id, node) in engine.nodes.iter() {
            if matches!(node, AppNode::MediaChannel(channel) if channel.kind() == kind) {
                selections.push(Self::selection(engine, id)?);
            }
        }
        Ok(Self {
            kind,
            publisher: manager.sender_catalog(kind).publisher()?,
            membership: engine.nodes.membership_revision(),
            selections,
            warning: None,
            processed_warning: false,
        })
    }
    fn refresh(&mut self, engine: &AppEngine) -> Result<(), String> {
        for selection in &mut self.selections {
            if !selection.guard.is_current() {
                *selection = Self::selection(engine, selection.channel)?;
                self.processed_warning = false;
            }
        }
        Ok(())
    }
    pub(crate) fn publish(&mut self, names: &[String], internal_names: &[String]) -> Result<(), String> {
        self.publish_discovery(names, internal_names, None)
    }

    fn publish_discovery(
        &mut self,
        names: &[String],
        internal_names: &[String],
        error: Option<Arc<str>>,
    ) -> Result<(), String> {
        let catalog = Snapshot::from_names(self.kind, names, internal_names);
        let options = catalog.options.clone();
        let mut updates = Vec::new();
        // A replacement inbox batch must include every channel: an earlier batch may still
        // be pending while one changed binding is refreshed against the older installed options.
        let warning_changed = self.kind == MediaKind::Ndi && (!self.processed_warning || self.warning != error);
        if warning_changed || self.selections.iter().any(|selection| selection.options != options) {
            for selection in &self.selections {
                updates.push(SelectionUpdate {
                    kind: self.kind,
                    channel: selection.channel,
                    node: selection.node,
                    uuid: selection.uuid,
                    value: ParamValue::Enum(selection.value.clone()),
                    options: options.clone(),
                    guard: selection.guard.clone(),
                    warning: error.clone(),
                });
            }
        }
        self.publisher.publish(updates, catalog)?;
        for selection in &mut self.selections {
            selection.options = options.clone();
        }
        self.warning = error;
        self.processed_warning = true;
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct Snapshot {
    pub options: ParameterEnumOptions,
    pub default_sender: Option<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self::new(&[], &[])
    }
}

impl Snapshot {
    fn new(names: &[String], internal_names: &[String]) -> Self {
        Self::from_names(MediaKind::SharedTexture, names, internal_names)
    }

    fn from_names(kind: MediaKind, names: &[String], internal_names: &[String]) -> Self {
        let mut entries = vec![option("", "None".into())];
        let mut seen = HashSet::from([""]);
        let internal = internal_names.iter().map(String::as_str).collect::<HashSet<_>>();
        let mut default_sender = None;
        for name in names
            .iter()
            .take(64)
            .filter(|name| !name.contains('\0') && (kind != MediaKind::SharedTexture || name.len() <= 255))
        {
            if !seen.insert(name.as_str()) {
                continue;
            }
            let label = if internal.contains(name.as_str()) {
                format!("{name} (internal)")
            } else {
                default_sender.get_or_insert_with(|| name.clone());
                name.clone()
            };
            entries.push(option(name, label));
        }
        Self {
            options: entries.into(),
            default_sender,
        }
    }
}

fn option(name: &str, label: String) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: name.into(),
        value: ParamValue::Enum(name.into()),
        label,
        tags: vec!["available".into()],
        ordering: None,
    }
}

#[cfg(all(windows, feature = "native-video"))]
pub(crate) mod native;
#[cfg(test)]
mod tests;
