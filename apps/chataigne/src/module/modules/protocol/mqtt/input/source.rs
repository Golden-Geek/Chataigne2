use super::*;
use golden_core::{
    edit::{EditPreconditions, NodeReadGuard},
    node::NodeUuid,
    parameter::ParamValue,
    process_ctx::{SubtreeRevision, SubtreeStructureRevision},
};

struct Field {
    id: NodeId,
    read: NodeReadGuard,
    revision: SubtreeRevision,
}

pub(super) struct Source {
    pub module: NodeId,
    module_uuid: NodeUuid,
    pub root: NodeId,
    root_uuid: NodeUuid,
    pub mode: Mode,
    reads: [NodeReadGuard; 4],
    fields: [Field; 2],
    bindings: [(NodeId, SubtreeStructureRevision); 2],
    pub lifetime: RevisionGuard,
}

impl Source {
    pub(super) fn capture(
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        lifetime: RevisionGuard,
    ) -> Result<Self, String> {
        let child = |parent, name| {
            snapshot
                .find_child_by_decl_id(parent, name)
                .ok_or_else(|| format!("MQTT input field {name} disappeared"))
        };
        let root = child(module, "values")?;
        let parameters = child(module, "parameters")?;
        let processing = child(parameters, "processing")?;
        let auto_add = child(processing, "auto_add")?;
        let payload = child(processing, "payload_mode")?;
        let mode = Mode::read(snapshot, auto_add, payload).ok_or("MQTT input processing fields are invalid")?;
        Ok(Self {
            module,
            module_uuid: snapshot.node(module).ok_or("MQTT input module disappeared")?.uuid,
            root,
            root_uuid: snapshot.node(root).ok_or("MQTT Values disappeared")?.uuid,
            mode,
            reads: [
                NodeReadGuard::capture_identity(snapshot, module).unwrap(),
                NodeReadGuard::capture_identity(snapshot, root).unwrap(),
                NodeReadGuard::capture_structure(snapshot, parameters).unwrap(),
                NodeReadGuard::capture_structure(snapshot, processing).unwrap(),
            ],
            fields: [
                Field {
                    id: auto_add,
                    read: NodeReadGuard::capture(snapshot, auto_add).unwrap(),
                    revision: snapshot.subtree_revision(auto_add).unwrap(),
                },
                Field {
                    id: payload,
                    read: NodeReadGuard::capture(snapshot, payload).unwrap(),
                    revision: snapshot.subtree_revision(payload).unwrap(),
                },
            ],
            bindings: [
                (parameters, snapshot.subtree_structure_revision(parameters).unwrap()),
                (processing, snapshot.subtree_structure_revision(processing).unwrap()),
            ],
            lifetime,
        })
    }
    pub(super) fn current(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        self.lifetime.is_current()
            && self
                .fields
                .iter()
                .all(|field| snapshot.subtree_revision(field.id) == Some(field.revision))
            && self
                .bindings
                .iter()
                .all(|(id, revision)| snapshot.subtree_structure_revision(*id) == Some(*revision))
            && snapshot
                .node(self.module)
                .is_some_and(|node| node.uuid == self.module_uuid && node.enabled)
            && snapshot
                .node(self.root)
                .is_some_and(|node| node.uuid == self.root_uuid && node.enabled && node.parent == Some(self.module))
            && snapshot.find_child_by_decl_id(self.module, "values") == Some(self.root)
            && Mode::read(snapshot, self.fields[0].id, self.fields[1].id) == Some(self.mode)
    }
    pub(super) fn preconditions(&self, snapshot: &ProcessTreeSnapshot) -> EditPreconditions {
        let mut reads = Vec::with_capacity(7);
        reads.extend(self.reads);
        reads.extend(self.fields.iter().map(|field| field.read));
        reads.push(NodeReadGuard::capture_structure(snapshot, self.root).unwrap());
        EditPreconditions::new(reads, vec![self.lifetime.clone()]).unwrap()
    }
}

impl Mode {
    pub(super) fn capture(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Option<Self> {
        let parameters = snapshot.find_child_by_decl_id(module, "parameters")?;
        let processing = snapshot.find_child_by_decl_id(parameters, "processing")?;
        Self::read(
            snapshot,
            snapshot.find_child_by_decl_id(processing, "auto_add")?,
            snapshot.find_child_by_decl_id(processing, "payload_mode")?,
        )
    }
    fn read(snapshot: &ProcessTreeSnapshot, auto_add: NodeId, payload: NodeId) -> Option<Self> {
        let ParamValue::Bool(auto_add) = snapshot.node(auto_add)?.param_value.as_ref()? else {
            return None;
        };
        let ParamValue::Enum(payload) = snapshot.node(payload)?.param_value.as_ref()? else {
            return None;
        };
        Some(Self {
            auto_add: *auto_add,
            payload: match payload.as_str() {
                "auto" => PayloadMode::Auto,
                "text" => PayloadMode::Text,
                "json" => PayloadMode::Json,
                "raw" => PayloadMode::Raw,
                _ => return None,
            },
        })
    }
}
