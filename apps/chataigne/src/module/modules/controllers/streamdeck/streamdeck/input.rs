use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};
use golden_core::{
    edit::{EditPreconditions, NodeReadGuard},
    node::NodeUuid,
    process_ctx::SubtreeStructureRevision,
};
#[derive(Clone)]
struct Expected {
    id: NodeId,
    uuid: NodeUuid,
    enabled: bool,
    version: Option<SubtreeStructureRevision>,
}
#[derive(Clone)]
pub(super) struct Route {
    pub(super) page: String,
    pub(super) target: Option<NodeId>,
    reads: Vec<NodeReadGuard>,
    expected: Vec<Expected>,
}
impl Route {
    pub(super) fn capture(
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        page: &str,
        parent: NodeId,
        index: usize,
        count: usize,
    ) -> Result<Self, String> {
        if page.len() > layout::MAX_LABEL {
            return Err("Stream Deck page id exceeds 512 bytes".into());
        }
        let target = if index < count {
            let id = snapshot
                .child_at(parent, index)
                .ok_or("Stream Deck input layout is not ready")?;
            if !matches!(
                snapshot.node(id).and_then(|n| n.param_value.as_ref()),
                Some(ParamValue::Bool(_))
            ) {
                return Err("Stream Deck input slot must be a boolean".into());
            }
            Some(id)
        } else {
            None
        };
        let mut reads = Vec::with_capacity(8);
        let mut expected = Vec::with_capacity(8);
        let parameters = snapshot
            .find_child_by_decl_id(module, "parameters")
            .ok_or("Stream Deck parameters were removed")?;
        let source = if page == paging::DEFAULT_PAGE_ID {
            snapshot.find_child_by_decl_id(parameters, KEYS_FOLDER)
        } else {
            paging::container_id(snapshot, parameters).and_then(|host| snapshot.find_child(host, page))
        }
        .ok_or("Stream Deck source page was removed")?;
        let mut source_current = Some(source);
        for _ in 0..4 {
            let Some(id) = source_current else {
                return Err("Stream Deck source page left the module".into());
            };
            if id == module {
                break;
            }
            let node = snapshot.node(id).ok_or("Stream Deck source page was removed")?;
            reads.push(
                NodeReadGuard::capture_structure(snapshot, id)
                    .ok_or("Stream Deck source page structure is unavailable")?,
            );
            expected.push(Expected {
                id,
                uuid: node.uuid,
                enabled: node.enabled,
                version: snapshot.subtree_structure_revision(id),
            });
            source_current = node.parent;
        }
        if source_current != Some(module) {
            return Err("Stream Deck source page hierarchy exceeds four levels".into());
        }
        let mut current = target.unwrap_or(parent);
        for _ in 0..8 {
            let node = snapshot.node(current).ok_or("Stream Deck input target was removed")?;
            if current == module {
                reads.push(
                    NodeReadGuard::capture_identity(snapshot, current)
                        .ok_or("Stream Deck module identity is unavailable")?,
                );
                expected.push(Expected {
                    id: current,
                    uuid: node.uuid,
                    enabled: node.enabled,
                    version: None,
                });
                return Ok(Self {
                    page: page.into(),
                    target,
                    reads,
                    expected,
                });
            }
            reads.push(
                NodeReadGuard::capture_structure(snapshot, current)
                    .ok_or("Stream Deck input structure is unavailable")?,
            );
            expected.push(Expected {
                id: current,
                uuid: node.uuid,
                enabled: node.enabled,
                version: snapshot.subtree_structure_revision(current),
            });
            current = node.parent.ok_or("Stream Deck input target left the module")?;
        }
        Err("Stream Deck input hierarchy exceeds eight levels".into())
    }
    pub(super) fn matches(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        self.expected.iter().all(|read| {
            snapshot
                .node(read.id)
                .is_some_and(|node| node.uuid == read.uuid && node.enabled == read.enabled && node.enabled)
                && read
                    .version
                    .is_none_or(|version| snapshot.subtree_structure_revision(read.id) == Some(version))
        })
    }
    #[cfg(test)]
    pub(super) fn fixture() -> Self {
        Self {
            page: paging::DEFAULT_PAGE_ID.into(),
            target: None,
            reads: Vec::new(),
            expected: Vec::new(),
        }
    }
    pub(super) fn fence(&self, ctx: &mut ProcessCtx) {
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(self.reads.clone(), vec![]).unwrap(),
        });
    }
}
pub(super) struct PreparedInput {
    pub(super) input: runtime::Input,
    pub(super) route: Route,
    callback: NativeCallbackPayload,
    pub(super) log: String,
    // A physical input keeps the original routing/feedback capture's admission alive.
    _frame: Option<golden_io::RetainedPayload<runtime::Frame>>,
}
pub(super) struct Draft {
    input: runtime::Input,
    route: Route,
    callback: NativeCallbackDraft,
    frame: Option<golden_io::RetainedPayload<runtime::Frame>>,
}
impl Draft {
    pub(super) fn new(
        input: runtime::Input,
        route: Route,
        frame: Option<golden_io::RetainedPayload<runtime::Frame>>,
    ) -> Result<Self, String> {
        let (index, pressed) = input.parts();
        if index >= runtime::MAX_KEYS {
            return Err("Stream Deck key index exceeds 63".into());
        }
        let callback = NativeCallbackDraft::new(
            if pressed {
                KEY_PRESSED_CALLBACK
            } else {
                KEY_RELEASED_CALLBACK
            },
            vec![serde_json::json!(index), serde_json::json!(route.page)],
        )?;
        Ok(Self {
            input,
            route,
            callback,
            frame,
        })
    }
    pub(super) fn finish(&self, callbacks: &NativeCallbackPreparer) -> Option<PreparedInput> {
        let (index, pressed) = self.input.parts();
        Some(PreparedInput {
            input: self.input,
            route: self.route.clone(),
            callback: callbacks.try_retain(&self.callback)?,
            log: format!(
                "Stream Deck key {index} {}",
                if pressed { "pressed" } else { "released" }
            ),
            _frame: self.frame.clone(),
        })
    }
}
impl StreamDeckModule {
    pub(super) fn route(&self, snapshot: &ProcessTreeSnapshot, index: usize) -> Result<Route, String> {
        let page = self.layout.active(
            snapshot,
            self.base
                .parameters_id()
                .ok_or("Stream Deck parameters are not ready")?,
        )?;
        let parent = self
            .layout
            .value_page(
                snapshot,
                self.base.values_id().ok_or("Stream Deck values are not ready")?,
                &page,
            )
            .ok_or("Stream Deck input page is not ready")?;
        Route::capture(snapshot, self.id(), &page, parent, index, self.model().key_count())
    }
    pub(super) fn script_input(&mut self, ctx: &mut ProcessCtx, index: usize, pressed: bool) -> Result<(), String> {
        if index >= runtime::MAX_KEYS {
            return Err("Stream Deck key index must be between 0 and 63".into());
        }
        let snapshot = ctx
            .tree_snapshot_arc()
            .ok_or("Stream Deck input requires an engine snapshot")?;
        let route = self.route(&snapshot, index)?;
        if !route.matches(&snapshot) {
            return Err("Stream Deck input is disabled or its target changed".into());
        }
        let pool = runtime::sources()?;
        let reservation = pool
            .try_reserve(runtime::INPUT_WEIGHT)
            .map_err(|_| "Stream Deck input lifetime capacity is full")?;
        let draft = Draft::new(
            if pressed {
                runtime::Input::Down(index)
            } else {
                runtime::Input::Up(index)
            },
            route,
            None,
        )?;
        let prepared = draft
            .finish(&NativeCallbackPreparer::new()?)
            .ok_or("Stream Deck callback lifetime capacity is full")?;
        let row = reservation
            .retain(prepared, runtime::INPUT_WEIGHT)
            .map_err(|_| "Stream Deck input exceeds its reservation")?;
        row.route.fence(ctx);
        self.project_input(ctx, &row);
        ctx.edits.retain_payload(row);
        Ok(())
    }
    pub(super) fn project_input(&mut self, ctx: &mut ProcessCtx, row: &PreparedInput) {
        let (_, pressed) = row.input.parts();
        if let Some(target) = row.route.target {
            ctx.set_param(target, ParamValue::Bool(pressed));
        }
        ctx.emit_custom_event(row.callback.event(self.id()));
        self.base.emit_incoming_traffic(ctx);
        if self.base.log_incoming_enabled() {
            golden_core::log!(origin=self.id();row.log.clone());
        }
    }
}
