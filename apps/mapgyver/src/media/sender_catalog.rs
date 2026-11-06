//! Live availability metadata; authored choices and recovery remain independent.
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    engine::NodeWaker,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEnumOption, ParameterEnumOptions},
    process_ctx::ProcessCtx,
};
use golden_io::RevisionGuard;
use std::sync::{Arc, Mutex, Weak};

struct State {
    waker: Option<NodeWaker>,
    pending: Option<Vec<SelectionUpdate>>,
    closed: bool,
    options: ParameterEnumOptions,
}
impl Default for State {
    fn default() -> Self {
        Self {
            waker: None,
            pending: None,
            closed: false,
            options: choices(&[]),
        }
    }
}
#[derive(Default)]
pub(crate) struct Inbox(Arc<Mutex<State>>);
impl Inbox {
    pub(super) fn options(&self) -> ParameterEnumOptions {
        self.0
            .lock()
            .map(|state| state.options.clone())
            .unwrap_or_else(|_| choices(&[]))
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
    fn publish(&self, updates: Vec<SelectionUpdate>, options: ParameterEnumOptions) -> Result<(), String> {
        let state = self.state.upgrade().ok_or("sender catalog retired")?;
        let mut state = state.lock().map_err(|_| "sender catalog inbox poisoned")?;
        if state.closed || self.waker.is_closed() {
            return Err("sender catalog retired".into());
        }
        // The first creation must see discovery even when no channel exists to receive metadata.
        state.options = options;
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
    node: NodeId,
    uuid: NodeUuid,
    value: ParamValue,
    options: ParameterEnumOptions,
    guard: RevisionGuard,
}
pub(crate) struct Source {
    publisher: Publisher,
    membership: (uuid::Uuid, u64),
    selections: Vec<Selection>,
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
        let manager = super::authoring::manager(engine)?;
        let Some(AppNode::MediaManager(manager)) = engine.nodes.get(manager) else {
            unreachable!()
        };
        let mut selections = Vec::new();
        for (id, node) in engine.nodes.iter() {
            if matches!(node, AppNode::MediaChannel(channel) if channel.kind() == super::protocol::MediaKind::SharedTexture)
            {
                selections.push(Self::selection(engine, id)?);
            }
        }
        Ok(Self {
            publisher: manager.sender_catalog.publisher()?,
            membership: engine.nodes.membership_revision(),
            selections,
        })
    }
    fn refresh(&mut self, engine: &AppEngine) -> Result<(), String> {
        for selection in &mut self.selections {
            if !selection.guard.is_current() {
                *selection = Self::selection(engine, selection.channel)?;
            }
        }
        Ok(())
    }
    pub(crate) fn publish(&mut self, names: &[String]) -> Result<(), String> {
        let options = choices(names);
        let mut updates = Vec::new();
        // A replacement inbox batch must include every channel: an earlier batch may still
        // be pending while one changed binding is refreshed against the older installed options.
        if self.selections.iter().any(|selection| selection.options != options) {
            for selection in &self.selections {
                updates.push(SelectionUpdate {
                    node: selection.node,
                    uuid: selection.uuid,
                    value: ParamValue::Enum(selection.value.clone()),
                    options: options.clone(),
                    guard: selection.guard.clone(),
                });
            }
        }
        self.publisher.publish(updates, options.clone())?;
        for selection in &mut self.selections {
            selection.options = options.clone();
        }
        Ok(())
    }
}

pub(crate) fn choices(names: &[String]) -> ParameterEnumOptions {
    let mut entries = vec![option("", "None".into(), true)];
    let mut seen = std::collections::HashSet::from([String::new()]);
    for name in names
        .iter()
        .take(64)
        .filter(|name| name.len() <= 255 && !name.contains('\0'))
    {
        if seen.insert(name.clone()) {
            entries.push(option(name, name.clone(), true));
        }
    }
    entries.into()
}
fn option(name: &str, label: String, available: bool) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: name.into(),
        value: ParamValue::Enum(name.into()),
        label,
        tags: if available {
            vec!["available".into()]
        } else {
            vec!["unavailable".into()]
        },
        ordering: None,
    }
}

#[cfg(all(windows, feature = "native-video"))]
pub(crate) mod native;
#[cfg(test)]
mod tests;
