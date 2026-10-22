use super::*;
use golden_io::RevisionGuard;

// A DFS step (including ascent) consumes service budget. Wide and deep authored
// command trees therefore cannot turn one catalog refresh into an actor scan.
#[derive(Debug)]
pub(super) struct Cursor {
    root: NodeId,
    tester: Option<NodeId>,
    at: Option<NodeId>,
    ascending: bool,
    first: bool,
    options: Vec<ParameterEnumOption>,
    guard: RevisionGuard,
}
impl Cursor {
    pub(super) fn new(
        snapshot: &ProcessTreeSnapshot,
        root: NodeId,
        options: Vec<ParameterEnumOption>,
        guard: RevisionGuard,
    ) -> Self {
        let tester = snapshot.find_child_by_decl_id(root, "command_tester");
        Self {
            root,
            tester,
            at: tester.or_else(|| snapshot.node(root).and_then(|node| node.first_child)),
            ascending: false,
            first: tester.is_some(),
            options,
            guard,
        }
    }
    fn step(&mut self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let id = self.at?;
        let boundary = if self.first { self.tester.unwrap() } else { self.root };
        let Some(node) = snapshot.node(id) else {
            self.at = None;
            return None;
        };
        let result = (!self.ascending).then_some(id);
        if !self.ascending && (self.first || Some(id) != self.tester) && node.first_child.is_some() {
            self.at = node.first_child;
        } else if id != boundary && node.next_sibling.is_some() {
            self.at = node.next_sibling;
            self.ascending = false;
        } else if id != boundary {
            self.at = node.parent;
            self.ascending = true;
        } else if self.first {
            self.first = false;
            self.at = snapshot.node(self.root).and_then(|node| node.first_child);
            self.ascending = false;
        } else {
            self.at = None;
        }
        result
    }
}
impl AppControlModule {
    pub(super) fn service_catalog(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(mut cursor) = self.service.catalog.take() else {
            return;
        };
        for _ in 0..64 {
            if cursor.at.is_none() {
                break;
            }
            let Some(id) = cursor.step(snapshot) else {
                continue;
            };
            if !snapshot
                .node(id)
                .is_some_and(|node| is_app_control_command_type(&node.node_type))
            {
                continue;
            }
            sync_command_watched_app_options(ctx, snapshot, id, &cursor.options, cursor.guard.clone());
        }
        if cursor.at.is_some() {
            self.service.catalog = Some(cursor);
            self.request_service();
        }
    }
}
