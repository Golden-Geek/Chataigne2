//! Latest-only native metadata handoff through Golden's lifetime-fenced worker wake contract.
use super::{
    display_matching::match_displays,
    protocol::{DisplayIdentityHint, DisplayInfo, ScreenId, ScreenTarget},
};
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    engine::NodeWaker,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEnumOption, ParameterEnumOptions},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

#[derive(Default)]
struct InboxState {
    waker: Option<NodeWaker>,
    pending: Option<Update>,
    closed: bool,
}

#[derive(Default)]
pub(crate) struct CatalogInbox {
    state: Arc<Mutex<InboxState>>,
    dirty: RevisionSource,
}

impl CatalogInbox {
    pub fn register(&self, ctx: &ProcessCtx) -> Result<(), String> {
        self.state.lock().map_err(|_| "display metadata inbox poisoned")?.waker =
            Some(ctx.node_waker().map_err(|error| error.to_string())?);
        self.invalidate();
        Ok(())
    }
    pub fn publisher(&self) -> Option<Publisher> {
        let state = self.state.lock().ok()?;
        Some(Publisher {
            state: Arc::downgrade(&self.state),
            waker: state.waker.clone()?,
        })
    }
    pub fn take(&self) -> Option<Update> {
        self.state.lock().ok()?.pending.take()
    }
    pub fn invalidate(&self) {
        self.dirty.advance();
    }
    pub fn guard(&self) -> RevisionGuard {
        let reader = self.dirty.reader();
        reader.guard_at(reader.current())
    }
}

impl Drop for CatalogInbox {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            state.pending.take();
        }
    }
}

#[derive(Clone)]
pub(crate) struct Publisher {
    state: Weak<Mutex<InboxState>>,
    waker: NodeWaker,
}

impl Publisher {
    pub fn is_closed(&self) -> bool {
        self.waker.is_closed()
            || self
                .state
                .upgrade()
                .is_none_or(|state| state.lock().map_or(true, |state| state.closed))
    }
    pub fn publish(&self, update: Update) -> Result<(), String> {
        let state = self.state.upgrade().ok_or("display metadata destination retired")?;
        let mut state = state.lock().map_err(|_| "display metadata inbox poisoned")?;
        if state.closed || self.waker.is_closed() {
            return Err("display metadata destination retired".into());
        }
        state.pending = Some(update);
        drop(state);
        self.waker.wake().map_err(|error| error.to_string())
    }
}

struct Selection {
    node: NodeId,
    uuid: NodeUuid,
    value: ParamValue,
    hint: DisplayIdentityHint,
    options: ParameterEnumOptions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredDisplay {
    pub intended: DisplayIdentityHint,
    pub display: DisplayInfo,
    pub reason: super::display_matching::DisplayMatchReason,
    pub ambiguous: bool,
}

/// Captured only after topology or authored target changes, never per frame.
pub(crate) struct Source {
    pub publisher: Publisher,
    pub guard: RevisionGuard,
    selections: Vec<Selection>,
    requests: Vec<(ScreenId, DisplayIdentityHint)>,
}

impl Source {
    pub fn capture(engine: &AppEngine) -> Result<Self, String> {
        let id = super::authoring::manager(engine)?;
        let Some(AppNode::ScreenManager(manager)) = engine.nodes.get(id) else {
            unreachable!()
        };
        let publisher = manager
            .catalog_publisher()
            .ok_or("display metadata receiver is not ready")?;
        let mut selections = Vec::new();
        let mut requests = Vec::new();
        let mut child = manager.node_data().first_child;
        while let Some(id) = child {
            let node = engine.nodes.get(id).ok_or("Screens child missing")?;
            child = node.node_data().next_sibling;
            let AppNode::Screen(screen) = node else { continue };
            if selections.len() == super::rendering::MAX_SCREEN_CATALOG {
                return Err("Screen catalog exceeds admitted count".into());
            }
            let parameter = screen.display_parameter_id();
            let leaf = engine.nodes.get(parameter).ok_or("Screen display Parameter missing")?;
            let hint = screen.display_hint();
            if screen.node_data().effective_enabled
                && screen.definition().enabled
                && matches!(screen.definition().target, Some(ScreenTarget::Fullscreen { .. }))
            {
                requests.push((ScreenId(screen.node_data().meta.uuid), hint.clone()));
            }
            let state = leaf.engine_param_snapshot().ok_or("display is not a Parameter")?;
            selections.push(Selection {
                node: parameter,
                uuid: leaf.node_data().meta.uuid,
                value: state.value,
                options: state.constraints.enum_options,
                hint,
            });
        }
        Ok(Self {
            publisher,
            guard: manager.catalog_inbox.guard(),
            selections,
            requests,
        })
    }

    /// Name preparation and cohort allocation belong on the native preparation worker.
    pub fn prepare(&self, displays: Arc<[DisplayInfo]>, options: &ParameterEnumOptions) -> Update {
        let mut entries = options.as_slice().to_vec();
        let mut identities = entries
            .iter()
            .map(|entry| entry.variant_id.clone())
            .collect::<std::collections::HashSet<_>>();
        for selection in &self.selections {
            if !selection.hint.identity.is_empty() && identities.insert(selection.hint.identity.clone()) {
                let label = if selection.hint.name.is_empty() {
                    "Saved display (offline)".to_owned()
                } else {
                    format!("{} (offline)", selection.hint.name)
                };
                entries.push(option(&selection.hint.identity, label, None));
            }
        }
        // Domains retain previously admitted tokens for exact Undo/Redo after hotplug.
        // Shared domains are visited once, independently of the number of Screens.
        let mut domains = std::collections::HashSet::new();
        for selection in &self.selections {
            if !domains.insert(Arc::as_ptr(selection.options.shared())) {
                continue;
            }
            for entry in selection.options.as_slice() {
                if !entry.variant_id.is_empty() && identities.insert(entry.variant_id.clone()) {
                    let mut entry = entry.clone();
                    if !entry.label.ends_with(" (offline)") {
                        entry.label.push_str(" (offline)");
                    }
                    entries.push(entry);
                }
            }
        }
        let shared = if entries.len() == options.as_slice().len() {
            options.clone()
        } else {
            entries.into()
        };
        let mut empty_entries = shared.as_slice().to_vec();
        empty_entries.push(option("", "No display selected".into(), None));
        let empty_options: ParameterEnumOptions = empty_entries.into();
        let resolved = match_displays(&self.requests, &displays)
            .into_iter()
            .map(|(screen, matched)| {
                let intended = self
                    .requests
                    .iter()
                    .find(|(id, _)| *id == screen)
                    .expect("matched request exists")
                    .1
                    .clone();
                (
                    screen,
                    RecoveredDisplay {
                        intended,
                        display: displays[matched.display_index].clone(),
                        reason: matched.reason,
                        ambiguous: matched.ambiguous,
                    },
                )
            })
            .collect();
        let choices = self
            .selections
            .iter()
            .map(|selection| {
                let choices = if selection.hint.identity.is_empty() || selection.options.contains_variant("") {
                    empty_options.clone()
                } else {
                    shared.clone()
                };
                Choice {
                    node: selection.node,
                    uuid: selection.uuid,
                    value: selection.value.clone(),
                    options: choices,
                }
            })
            .collect();
        Update {
            guard: self.guard.clone(),
            profiles: self
                .selections
                .iter()
                .filter_map(|selection| {
                    selection.hint.bounds.map(|bounds| DisplayInfo {
                        refresh_rate: None,
                        id: selection.hint.identity.clone(),
                        name: selection.hint.name.clone(),
                        edid: selection.hint.edid.clone(),
                        index: selection.hint.index,
                        bounds,
                        primary: false,
                    })
                })
                .collect(),
            displays,
            resolved,
            choices,
        }
    }
}

struct Choice {
    node: NodeId,
    uuid: NodeUuid,
    value: ParamValue,
    options: ParameterEnumOptions,
}

pub(crate) struct Update {
    pub guard: RevisionGuard,
    pub displays: Arc<[DisplayInfo]>,
    pub profiles: Vec<DisplayInfo>,
    pub resolved: HashMap<ScreenId, RecoveredDisplay>,
    choices: Vec<Choice>,
}

impl Update {
    pub fn apply(self, ctx: &mut ProcessCtx) {
        for choice in self.choices {
            ctx.edits.push(Edit::SetParamEnumOptions {
                node: choice.node,
                expected_uuid: choice.uuid,
                expected_value: choice.value,
                selected_value: None,
                options: choice.options,
            });
        }
    }
}

pub(crate) fn display_options(displays: &[DisplayInfo]) -> ParameterEnumOptions {
    let mut ordered = displays.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|display| (display.index, display.bounds.left, display.bounds.top, &display.id));
    ordered
        .into_iter()
        .map(|display| {
            let name = if display.name.trim().is_empty() {
                "Display"
            } else {
                display.name.as_str()
            };
            let label = if display.index == 0 {
                name.to_owned()
            } else {
                format!("{name} · {}", display.index)
            };
            option(&display.id, label, i32::try_from(display.index).ok())
        })
        .collect::<Vec<_>>()
        .into()
}

fn option(id: &str, label: String, ordering: Option<i32>) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: id.into(),
        value: ParamValue::Enum(id.into()),
        label,
        tags: vec![],
        ordering,
    }
}
