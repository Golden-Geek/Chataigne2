use super::*;
use golden_core::edit::{EditCompletion, EditCompletionStatus, EditPreconditions, EditQueue, NodeReadGuard};
use golden_io::{PayloadPool, RetainedPayload, RevisionSource, WorkerNotifier};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};
mod builders;
pub(super) const MAX_PAGES: usize = 512;
pub(super) const MAX_LABEL: usize = 512;
const AUTHOR_WEIGHT: usize = 1024 * 1024;
#[derive(Clone, Copy)]
pub(super) struct Owner {
    pub(super) module: NodeId,
    pub(super) parameters: NodeId,
    pub(super) values: NodeId,
    pub(super) count: usize,
    pub(super) model: NodeId,
}
struct Page {
    node: NodeId,
    id: String,
    label: String,
}
enum Phase {
    Defaults,
    Claims(usize),
    Gather(usize),
    Pages(usize),
    Cleanup(usize),
    Selector,
}
struct Scan {
    count: usize,
    phase: Phase,
    pages: Vec<Page>,
    claimed: Vec<String>,
}
enum Action {
    Add {
        label: String,
        template: builders::Template,
    },
    Remove(String),
}
struct Request {
    root: NodeReadGuard,
    action: Action,
}
pub(super) struct Layout {
    dirty: bool,
    again: bool,
    scan: Option<Scan>,
    pending: Option<EditCompletion>,
    life: RevisionSource,
    generation: u64,
    requests: VecDeque<RetainedPayload<Request>>,
    pub(super) ready: bool,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            dirty: true,
            again: false,
            scan: None,
            pending: None,
            life: RevisionSource::default(),
            generation: 0,
            requests: VecDeque::new(),
            ready: false,
        }
    }
}
fn pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("streamdeck-authoring", 64, 64 * AUTHOR_WEIGHT).map_err(|e| e.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
struct WakeOnDrop(WorkerNotifier);
impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        self.0.notify();
    }
}
impl Layout {
    pub(super) fn mark(&mut self) {
        if self.scan.is_some() {
            self.again = true;
        } else {
            self.dirty = true;
        }
    }
    pub(super) fn cancel(&mut self) {
        self.generation = self.life.advance();
        self.scan = None;
        self.pending = None;
        self.requests.clear();
        self.dirty = true;
        self.again = false;
        self.ready = false;
    }
    pub(super) fn waiting(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|r| r.status() == EditCompletionStatus::Pending)
    }
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some() || self.dirty || self.scan.is_some() || !self.requests.is_empty()
    }
    pub(super) fn add(&mut self, snapshot: &ProcessTreeSnapshot, owner: Owner, name: &str) -> Result<(), String> {
        if name.len() > MAX_LABEL {
            return Err("Stream Deck page name exceeds 512 bytes".into());
        }
        if self.requests.len() >= 64 {
            return Err("Stream Deck authoring queue is full".into());
        }
        if paging::container_id(snapshot, owner.parameters)
            .is_some_and(|id| snapshot.child_ids_slice(id).len() >= MAX_PAGES)
        {
            return Err("Stream Deck supports at most 512 derived pages".into());
        }
        let reservation = pool()?
            .try_reserve(AUTHOR_WEIGHT)
            .map_err(|_| "Stream Deck authoring lifetime capacity is full")?;
        let root = snapshot
            .find_child_by_decl_id(owner.parameters, KEYS_FOLDER)
            .ok_or("Stream Deck default layout is unavailable")?;
        let template = builders::Template::capture(snapshot, root, owner.count)?;
        let root = NodeReadGuard::capture_identity(snapshot, owner.module)
            .ok_or("Stream Deck module identity is unavailable")?;
        let label = if name.trim().is_empty() { "Page" } else { name.trim() }.to_string();
        self.requests.push_back(
            reservation
                .retain(
                    Request {
                        root,
                        action: Action::Add { label, template },
                    },
                    AUTHOR_WEIGHT,
                )
                .map_err(|_| "Stream Deck page request exceeds its reservation")?,
        );
        Ok(())
    }
    pub(super) fn remove(&mut self, snapshot: &ProcessTreeSnapshot, owner: Owner, id: &str) -> Result<(), String> {
        if id.len() > MAX_LABEL {
            return Err("Stream Deck page id exceeds 512 bytes".into());
        }
        if id == paging::DEFAULT_PAGE_ID {
            return Ok(());
        }
        if self.requests.len() >= 64 {
            return Err("Stream Deck authoring queue is full".into());
        }
        let root = NodeReadGuard::capture_identity(snapshot, owner.module)
            .ok_or("Stream Deck module identity is unavailable")?;
        self.requests.push_back(
            pool()?
                .try_retain(
                    Request {
                        root,
                        action: Action::Remove(id.into()),
                    },
                    AUTHOR_WEIGHT,
                )
                .map_err(|_| "Stream Deck authoring lifetime capacity is full")?,
        );
        Ok(())
    }
    pub(super) fn active(&self, snapshot: &ProcessTreeSnapshot, parameters: NodeId) -> Result<String, String> {
        let value = snapshot
            .find_child_by_decl_id(parameters, paging::ACTIVE_PAGE_PARAM)
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_value.as_ref());
        let Some(ParamValue::Enum(value)) = value else {
            return Ok(paging::DEFAULT_PAGE_ID.into());
        };
        if value.len() > MAX_LABEL {
            return Err("Stream Deck active page exceeds 512 bytes".into());
        }
        if value == paging::DEFAULT_PAGE_ID || value.is_empty() {
            return Ok(paging::DEFAULT_PAGE_ID.into());
        }
        if paging::container_id(snapshot, parameters)
            .and_then(|parent| snapshot.find_child(parent, value))
            .is_some()
        {
            Ok(value.clone())
        } else {
            Ok(paging::DEFAULT_PAGE_ID.into())
        }
    }
    pub(super) fn value_page(&self, snapshot: &ProcessTreeSnapshot, values: NodeId, page: &str) -> Option<NodeId> {
        if page == paging::DEFAULT_PAGE_ID {
            snapshot.find_child_by_decl_id(values, KEYS_FOLDER)
        } else {
            paging::container_id(snapshot, values).and_then(|parent| snapshot.find_child(parent, page))
        }
    }
    pub(super) fn control_page(
        &self,
        snapshot: &ProcessTreeSnapshot,
        parameters: NodeId,
        page: &str,
    ) -> Option<NodeId> {
        if page == paging::DEFAULT_PAGE_ID {
            snapshot.find_child_by_decl_id(parameters, KEYS_FOLDER)
        } else {
            paging::container_id(snapshot, parameters).and_then(|parent| snapshot.find_child(parent, page))
        }
    }
    pub(super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        owner: Owner,
        notifier: WorkerNotifier,
    ) -> Result<(), String> {
        if let Some(receipt) = self.pending.take() {
            match receipt.status() {
                EditCompletionStatus::Pending => {
                    self.pending = Some(receipt);
                    return Ok(());
                }
                EditCompletionStatus::Failed => {
                    self.scan = None;
                    self.dirty = true;
                }
                EditCompletionStatus::Applied => {}
            }
        }
        if !self.busy() {
            return Ok(());
        }
        if snapshot.find_child_by_decl_id(owner.module, "parameters") != Some(owner.parameters)
            || snapshot.find_child_by_decl_id(owner.module, "values") != Some(owner.values)
            || snapshot.find_child_by_decl_id(owner.parameters, "model") != Some(owner.model)
        {
            return Err("Stream Deck layout bindings left their declared module paths".into());
        }
        let mut edits = EditQueue::new();
        let mut retained_request = None;
        let root = NodeReadGuard::capture_identity(snapshot, owner.module).ok_or("Stream Deck module was removed")?;
        let mut reads = vec![root];
        let mut guard_model = true;
        for id in [owner.parameters, owner.values] {
            reads.push(NodeReadGuard::capture_structure(snapshot, id).ok_or("Stream Deck layout parent was removed")?);
        }
        let host = paging::container_id(snapshot, owner.parameters);
        if let Some(host) = host {
            reads.push(NodeReadGuard::capture_structure(snapshot, host).ok_or("Stream Deck page host was removed")?);
        }
        if let Some(container) = paging::container_id(snapshot, owner.values) {
            reads.push(
                NodeReadGuard::capture_structure(snapshot, container)
                    .ok_or("Stream Deck mirror container was removed")?,
            );
        }
        let selector = snapshot.find_child_by_decl_id(owner.parameters, paging::ACTIVE_PAGE_PARAM);
        if host.is_none() {
            let mut node = crate::app::PageHost::new();
            node.node_data_mut().meta.label = "Pages".into();
            node.node_data_mut().meta.short_name = paging::PAGES_CONTAINER.into();
            node.node_data_mut().meta.decl_id = golden_core::node::DeclId(paging::PAGES_CONTAINER.into());
            edits.push(Edit::AddNodeTree {
                parent: owner.parameters,
                prev_sibling: None,
                tree: NodeTree::new(node),
            });
        }
        if selector.is_none() {
            edits.push(Edit::AddNodeTree {
                parent: owner.parameters,
                prev_sibling: None,
                tree: NodeTree::new(builders::selector()),
            });
        }
        let reservation = if let Some(request) = self.requests.front() {
            retained_request = Some(request.clone());
            None
        } else {
            Some(
                pool()?
                    .try_reserve(AUTHOR_WEIGHT)
                    .map_err(|_| "Stream Deck layout capacity is full")?,
            )
        };
        if edits.pending.is_empty() {
            let host = host.unwrap();
            if snapshot.child_ids_slice(host).len() > MAX_PAGES {
                self.scan = None;
                self.dirty = false;
                return Err("Stream Deck supports at most 512 derived pages".into());
            }
            if let Some(request) = self.requests.pop_front() {
                guard_model = false;
                reads.push(request.root);
                self.dirty = true;
                match &request.action {
                    Action::Add { label, template } => {
                        if snapshot.child_ids_slice(host).len() >= MAX_PAGES {
                            return Err("Stream Deck supports at most 512 derived pages".into());
                        }
                        let mut claimed = vec![paging::DEFAULT_PAGE_ID.to_string()];
                        for id in snapshot.child_ids_slice(host) {
                            let node = snapshot.node(*id).unwrap();
                            if node.short_name.len() > MAX_LABEL {
                                return Err("Stream Deck page id exceeds 512 bytes".into());
                            }
                            claimed.push(paging::page_id_of(node));
                        }
                        let mut slug = paging::sanitize_page_id(label);
                        slug.truncate(504);
                        let id = paging::unique_id(&slug, &claimed);
                        let mut folder = builders::authored_folder(label);
                        folder.node_data_mut().meta.short_name = id;
                        let mut tree = NodeTree::new(folder);
                        template.append(&mut tree);
                        edits.push(Edit::AddNodeTree {
                            parent: host,
                            prev_sibling: None,
                            tree,
                        });
                    }
                    Action::Remove(id) => {
                        if let Some(node) = snapshot.find_child(host, id) {
                            edits.push(Edit::RemoveNode { node });
                        }
                    }
                }
                retained_request = Some(request);
            } else {
                if self.scan.as_ref().is_some_and(|scan| scan.count != owner.count) {
                    self.scan = None;
                    self.dirty = true;
                }
                if self.scan.is_none() && self.dirty {
                    self.dirty = false;
                    self.again = false;
                    self.scan = Some(Scan {
                        count: owner.count,
                        phase: Phase::Defaults,
                        pages: Vec::new(),
                        claimed: vec![paging::DEFAULT_PAGE_ID.into()],
                    });
                }
                if let Some(mut scan) = self.scan.take() {
                    let mut finished = false;
                    let default = snapshot
                        .find_child_by_decl_id(owner.parameters, KEYS_FOLDER)
                        .ok_or("Stream Deck controls are unavailable")?;
                    let values = snapshot
                        .find_child_by_decl_id(owner.values, KEYS_FOLDER)
                        .ok_or("Stream Deck values are unavailable")?;
                    reads.push(NodeReadGuard::capture_structure(snapshot, default).unwrap());
                    reads.push(NodeReadGuard::capture_structure(snapshot, values).unwrap());
                    match scan.phase {
                        Phase::Defaults => {
                            builders::resize(snapshot, &mut edits, default, owner.count, true, true)?;
                            builders::resize(snapshot, &mut edits, values, owner.count, false, false)?;
                            scan.phase = Phase::Claims(0);
                        }
                        Phase::Claims(start) => {
                            let children = snapshot.child_ids_slice(host);
                            for id in children.iter().skip(start).take(64) {
                                let node = snapshot.node(*id).unwrap();
                                if node.short_name.len() > MAX_LABEL || node.label.len() > MAX_LABEL {
                                    return Err("Stream Deck page metadata exceeds 512 bytes".into());
                                }
                                if node.child_count > 0 {
                                    scan.claimed.push(paging::page_id_of(node));
                                }
                            }
                            scan.phase = if start + 64 >= children.len() {
                                Phase::Gather(0)
                            } else {
                                Phase::Claims(start + 64)
                            };
                        }
                        Phase::Gather(start) => {
                            let children = snapshot.child_ids_slice(host);
                            let mut cursor = start;
                            for id in children.iter().skip(start).take(64) {
                                let node = snapshot.node(*id).unwrap();
                                if node.short_name.len() > MAX_LABEL || node.label.len() > MAX_LABEL {
                                    return Err("Stream Deck page metadata exceeds 512 bytes".into());
                                }
                                let id_string = if node.child_count == 0 {
                                    let mut slug = paging::sanitize_page_id(&node.label);
                                    slug.truncate(504);
                                    let page_id = paging::unique_id(&slug, &scan.claimed);
                                    scan.claimed.push(page_id.clone());
                                    if node.short_name != page_id {
                                        edits.push(Edit::PatchMeta {
                                            node: *id,
                                            patch: NodeMetaPatch {
                                                short_name: Some(page_id.clone()),
                                                ..Default::default()
                                            },
                                        });
                                    }
                                    page_id
                                } else {
                                    paging::page_id_of(node)
                                };
                                scan.pages.push(Page {
                                    node: *id,
                                    id: id_string,
                                    label: node.label.clone(),
                                });
                                cursor += 1;
                                if !edits.pending.is_empty() {
                                    break;
                                }
                            }
                            scan.phase = if cursor >= children.len() {
                                Phase::Pages(0)
                            } else {
                                Phase::Gather(cursor)
                            };
                        }
                        Phase::Pages(index) => {
                            if let Some(page) = scan.pages.get(index) {
                                if let Some(node) = snapshot.node(page.node).filter(|node| node.parent == Some(host)) {
                                    reads.push(NodeReadGuard::capture_structure(snapshot, page.node).unwrap());
                                    if node.child_count == 0 {
                                        builders::Template::capture(snapshot, default, owner.count)?
                                            .append_to(&mut edits, page.node);
                                    } else {
                                        builders::resize(snapshot, &mut edits, page.node, owner.count, true, false)?;
                                    }
                                    match paging::container_id(snapshot, owner.values) {
                                        None => {
                                            let mut tree = NodeTree::new(builders::locked_folder(
                                                "Pages",
                                                paging::PAGES_CONTAINER,
                                            ));
                                            let mut mirror =
                                                NodeTree::new(builders::locked_folder(&page.label, &page.id));
                                            for slot in 0..owner.count {
                                                mirror.push_child(builders::value_key(slot));
                                            }
                                            tree.push_child(mirror);
                                            edits.push(Edit::AddNodeTree {
                                                parent: owner.values,
                                                prev_sibling: None,
                                                tree,
                                            });
                                        }
                                        Some(container) => match snapshot.find_child(container, &page.id) {
                                            None => {
                                                let mut tree =
                                                    NodeTree::new(builders::locked_folder(&page.label, &page.id));
                                                for slot in 0..owner.count {
                                                    tree.push_child(builders::value_key(slot));
                                                }
                                                edits.push(Edit::AddNodeTree {
                                                    parent: container,
                                                    prev_sibling: None,
                                                    tree,
                                                });
                                            }
                                            Some(mirror) => {
                                                if snapshot.node(mirror).is_some_and(|n| n.label != page.label) {
                                                    edits.push(Edit::PatchMeta {
                                                        node: mirror,
                                                        patch: NodeMetaPatch {
                                                            label: Some(page.label.clone()),
                                                            ..Default::default()
                                                        },
                                                    });
                                                }
                                                builders::resize(
                                                    snapshot,
                                                    &mut edits,
                                                    mirror,
                                                    owner.count,
                                                    false,
                                                    false,
                                                )?;
                                            }
                                        },
                                    }
                                }
                            }
                            scan.phase = if index + 1 >= scan.pages.len() {
                                Phase::Cleanup(0)
                            } else {
                                Phase::Pages(index + 1)
                            };
                        }
                        Phase::Cleanup(start) => {
                            if let Some(container) = paging::container_id(snapshot, owner.values) {
                                let children = snapshot.child_ids_slice(container);
                                if children.len() > MAX_PAGES {
                                    return Err("Stream Deck mirror exceeds 512 pages".into());
                                }
                                if scan.pages.is_empty() {
                                    edits.push(Edit::RemoveNode { node: container });
                                    scan.phase = Phase::Selector;
                                } else {
                                    let mut removed = 0;
                                    for id in children.iter().skip(start).take(16) {
                                        if snapshot.node(*id).is_some_and(|node| {
                                            !scan.pages.iter().any(|page| page.id == node.short_name)
                                        }) {
                                            edits.push(Edit::RemoveNode { node: *id });
                                            removed += 1;
                                        }
                                    }
                                    scan.phase = if start + 16 >= children.len() {
                                        Phase::Selector
                                    } else {
                                        Phase::Cleanup(start + 16 - removed)
                                    };
                                }
                            } else {
                                scan.phase = Phase::Selector;
                            }
                        }
                        Phase::Selector => {
                            let unchanged = snapshot.child_ids_slice(host).len() == scan.pages.len()
                                && scan.pages.iter().all(|page| {
                                    snapshot
                                        .node(page.node)
                                        .is_some_and(|node| node.short_name == page.id && node.label == page.label)
                                });
                            if !unchanged {
                                self.dirty = true;
                                self.scan = None;
                                return Ok(());
                            }
                            let mut options = vec![super::enum_option(paging::DEFAULT_PAGE_ID, "Default", 0)];
                            options.extend(
                                scan.pages
                                    .iter()
                                    .enumerate()
                                    .map(|(i, p)| super::enum_option(&p.id, &p.label, i as i32 + 1)),
                            );
                            let selector = selector.unwrap();
                            let node = snapshot.node(selector).unwrap();
                            let Some(current) = &node.param_value else {
                                return Err("Stream Deck page selector is not a parameter".into());
                            };
                            let next = match current {
                                ParamValue::Enum(id)
                                    if id == paging::DEFAULT_PAGE_ID || scan.pages.iter().any(|p| p.id == *id) =>
                                {
                                    current.clone()
                                }
                                _ => ParamValue::Enum(paging::DEFAULT_PAGE_ID.into()),
                            };
                            if node
                                .param_constraints
                                .as_ref()
                                .is_none_or(|c| c.enum_options.as_slice() != options.as_slice())
                                || *current != next
                            {
                                reads.push(NodeReadGuard::capture(snapshot, selector).unwrap());
                                for page in &scan.pages {
                                    reads.push(NodeReadGuard::capture_structure(snapshot, page.node).unwrap());
                                }
                                edits.push(Edit::SetParamEnumOptions {
                                    node: selector,
                                    expected_uuid: node.uuid,
                                    expected_value: current.clone(),
                                    selected_value: Some(next),
                                    options: options.into(),
                                });
                            }
                            self.ready = true;
                            self.dirty = self.again;
                            self.scan = None;
                            finished = true;
                        }
                    }
                    if !finished {
                        self.scan = Some(scan);
                    }
                }
            }
        }
        if !edits.pending.is_empty() {
            if guard_model {
                reads.push(NodeReadGuard::capture(snapshot, owner.model).ok_or("Stream Deck model was removed")?);
            }
            let mut guarded = EditQueue::new();
            for chunk in reads.chunks(32) {
                guarded.push(Edit::CheckPreconditions {
                    preconditions: EditPreconditions::new(
                        chunk.to_vec(),
                        vec![self.life.reader().guard_at(self.generation)],
                    )
                    .unwrap(),
                });
            }
            guarded.pending.extend(edits.drain());
            let wake = WakeOnDrop(notifier);
            guarded.push(Edit::CallNodeMutation {
                node: owner.module,
                needs_tree_snapshot: false,
                callback: Box::new(move |_, _| {
                    drop(wake);
                    Ok(())
                }),
            });
            self.pending = Some(guarded.completion());
            if let Some(request) = retained_request {
                guarded.retain_payload(request);
            } else if let Some(reservation) = reservation {
                guarded.retain_payload(reservation.retain((), AUTHOR_WEIGHT).unwrap());
            }
            ctx.edits.pending.extend(guarded.drain());
        }
        Ok(())
    }
}
