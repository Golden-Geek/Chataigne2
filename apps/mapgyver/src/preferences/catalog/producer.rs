//! Latest-only, lifetime-fenced catalog delivery. Never changes the authored selection.
use super::super::{AUTOMATIC, VIDEO_DECL_ID};
use super::{Inbox, State, Update};
use crate::app::{AppEngine, AppNode};
use golden_core::{
    engine::NodeWaker,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEnumOption, ParameterEnumOptions},
};
use std::sync::{Arc, Mutex, Weak};

impl Inbox {
    fn publisher(&self) -> Result<Publisher, String> {
        let state = self.0.lock().map_err(|_| "GPU catalog inbox poisoned")?;
        Ok(Publisher {
            state: Arc::downgrade(&self.0),
            waker: state.waker.clone().ok_or("GPU Preferences not ready")?,
        })
    }
}
struct Publisher {
    state: Weak<Mutex<State>>,
    waker: NodeWaker,
}
impl Publisher {
    fn publish(&self, update: Update) -> Result<(), String> {
        let state = self.state.upgrade().ok_or("GPU Preferences retired")?;
        let mut state = state.lock().map_err(|_| "GPU catalog inbox poisoned")?;
        if state.closed || self.waker.is_closed() {
            return Err("GPU Preferences retired".into());
        }
        state.pending = Some(update);
        drop(state);
        self.waker.wake().map_err(|error| error.to_string())
    }
}

pub(crate) struct Source {
    publisher: Publisher,
    node: NodeId,
    uuid: NodeUuid,
    pub value: String,
    options: ParameterEnumOptions,
}
impl Source {
    pub fn capture(engine: &AppEngine) -> Result<Self, String> {
        let root = golden_core::app::live_preferences_root(engine).ok_or("Preferences missing")?;
        let mut child = engine
            .nodes
            .get(root)
            .ok_or("Preferences missing")?
            .node_data()
            .first_child;
        let mut remaining = engine.nodes.len();
        while let Some(id) = child {
            if remaining == 0 {
                return Err("Preferences sibling chain is cyclic".into());
            }
            remaining -= 1;
            let node = engine.nodes.get(id).ok_or("Preferences child missing")?;
            child = node.node_data().next_sibling;
            if node.node_data().meta.decl_id.0 != VIDEO_DECL_ID {
                continue;
            }
            let AppNode::VideoPreferences(video) = node else {
                return Err("Video Preferences type mismatch".into());
            };
            let parameter = video.adapter.id();
            let leaf = engine.nodes.get(parameter).ok_or("GPU Parameter missing")?;
            let state = leaf.engine_param_snapshot().ok_or("GPU selection is not a Parameter")?;
            let ParamValue::Enum(value) = state.value else {
                return Err("GPU selection is not an Enum".into());
            };
            return Ok(Self {
                publisher: video.inbox.publisher()?,
                node: parameter,
                uuid: leaf.node_data().meta.uuid,
                value,
                options: state.constraints.enum_options,
            });
        }
        Err("Video Preferences missing".into())
    }

    pub fn publish(&self, available: &[(String, String)]) -> Result<(), String> {
        let options = choices(available, &self.value, &self.options)?;
        if options == self.options {
            return Ok(());
        }
        self.publisher.publish(Update {
            node: self.node,
            uuid: self.uuid,
            value: ParamValue::Enum(self.value.clone()),
            options,
        })
    }
}

pub(crate) fn choices(
    available: &[(String, String)],
    selected: &str,
    previous: &ParameterEnumOptions,
) -> Result<ParameterEnumOptions, String> {
    let mut entries = vec![option(AUTOMATIC, "Automatic (high performance)".into())];
    let mut seen = std::collections::HashSet::from([AUTOMATIC.to_owned()]);
    for (id, label) in available {
        if seen.insert(id.clone()) {
            entries.push(option(id, label.clone()));
        }
    }
    // Retain historical tokens for exact Undo/Redo after device disappearance.
    for entry in previous.as_slice() {
        if seen.insert(entry.variant_id.clone()) {
            let label = entry.label.strip_suffix(" (unavailable)").unwrap_or(&entry.label);
            entries.push(option(&entry.variant_id, format!("{label} (unavailable)")));
        }
    }
    if seen.insert(selected.to_owned()) {
        entries.push(option(selected, format!("{selected} (unavailable)")));
    }
    if entries.len() > 64 {
        return Err("GPU Preferences exceeds 64 retained adapter choices".into());
    }
    Ok(entries.into())
}
fn option(id: &str, label: String) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: id.into(),
        value: ParamValue::Enum(id.into()),
        label,
        tags: vec![],
        ordering: None,
    }
}
