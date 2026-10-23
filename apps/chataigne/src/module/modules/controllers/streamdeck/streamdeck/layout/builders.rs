use super::*;
use golden_core::{
    node::{DeclId, NodeUserPermissions},
    parameter::ParameterConstraints,
};
pub(super) const TEMPLATE_BYTES: usize = 256 * 1024;
#[derive(Clone)]
pub(super) struct Template {
    nodes: Vec<TemplateNode>,
}
#[derive(Clone)]
struct TemplateNode {
    label: String,
    id: String,
    short_name: String,
    value: Option<ParamValue>,
    constraints: Option<ParameterConstraints>,
    children: Vec<TemplateNode>,
}
impl Template {
    pub(super) fn capture(snapshot: &ProcessTreeSnapshot, root: NodeId, count: usize) -> Result<Self, String> {
        let mut total = 0;
        let mut nodes = 0;
        let mut materialized = 0;
        for child in snapshot.child_ids_slice(root) {
            measure(snapshot, *child, 0, &mut total, &mut nodes, &mut materialized)?;
        }
        if snapshot.child_ids_slice(root).is_empty() {
            return Ok(Self {
                nodes: (0..count).map(|slot| default_template(slot, false)).collect(),
            });
        }
        Ok(Self {
            nodes: snapshot
                .child_ids_slice(root)
                .iter()
                .filter_map(|id| copy(snapshot, *id))
                .collect(),
        })
    }
    pub(super) fn append(&self, tree: &mut NodeTree) {
        for node in &self.nodes {
            tree.push_child(node.materialize());
        }
    }
    pub(super) fn append_to(&self, edits: &mut golden_core::edit::EditQueue, parent: NodeId) {
        for node in &self.nodes {
            edits.push(Edit::AddNodeTree {
                parent,
                prev_sibling: None,
                tree: node.materialize(),
            });
        }
    }
}
fn measure(
    snapshot: &ProcessTreeSnapshot,
    id: NodeId,
    depth: usize,
    bytes: &mut usize,
    nodes: &mut usize,
    materialized: &mut usize,
) -> Result<(), String> {
    let node = snapshot.node(id).ok_or("Stream Deck template node was removed")?;
    *nodes += 1;
    if *nodes > 256 || depth > 16 {
        return Err("Stream Deck page template exceeds 256 nodes / 16 levels".into());
    }
    if node.label.len() > MAX_LABEL || node.short_name.len() > MAX_LABEL || node.decl_id.len() > 4096 {
        return Err("Stream Deck template metadata exceeds bounds".into());
    }
    let strings = node.label.len() + node.short_name.len() + node.decl_id.len();
    *bytes = bytes.saturating_add(std::mem::size_of::<TemplateNode>() + strings);
    *materialized = materialized.saturating_add(
        std::mem::size_of::<NodeTree>()
            + std::mem::size_of::<Parameter>().max(std::mem::size_of::<Folder>())
            + 3 * strings,
    );
    if let Some(value) = &node.param_value {
        *bytes = bytes.saturating_add(value.retained_heap_bytes());
        *materialized = materialized.saturating_add(3 * value.retained_heap_bytes());
    }
    if let Some(constraints) = &node.param_constraints {
        let size = constraints_bytes(constraints)?;
        *bytes = bytes.saturating_add(size);
        *materialized = materialized.saturating_add(size);
    }
    if *bytes > TEMPLATE_BYTES {
        return Err("Stream Deck page template exceeds 256 KiB".into());
    }
    if bytes.saturating_add(*materialized) > AUTHOR_WEIGHT - 4096 {
        return Err("Stream Deck template and materialized page exceed the 1 MiB authoring budget".into());
    }
    for child in snapshot.child_ids_slice(id) {
        measure(snapshot, *child, depth + 1, bytes, nodes, materialized)?;
    }
    Ok(())
}

fn constraints_bytes(constraints: &ParameterConstraints) -> Result<usize, String> {
    struct Size(usize);
    impl std::io::Write for Size {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            if self.0 > TEMPLATE_BYTES / 16 {
                return Err(std::io::Error::other(
                    "Stream Deck template constraints exceed retained bounds",
                ));
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    // String/Vec headers can cost more than their JSON spelling. This conservative
    // factor also covers empty string lists, without cloning arbitrary authored filters.
    let mut size = Size(0);
    serde_json::to_writer(&mut size, constraints)
        .map_err(|_| "Stream Deck template constraints exceed retained bounds")?;
    Ok(size.0 * 16 + constraints.enum_options.retained_bytes())
}
fn copy(snapshot: &ProcessTreeSnapshot, id: NodeId) -> Option<TemplateNode> {
    let node = snapshot.node(id)?;
    if node.short_name == "unpaged" || node.decl_id.rsplit('/').next() == Some("unpaged") || node.label == "Unpaged" {
        return None;
    }
    Some(TemplateNode {
        label: node.label.clone(),
        id: node.decl_id.clone(),
        short_name: node.short_name.clone(),
        value: node.param_value.clone(),
        constraints: node.param_constraints.clone(),
        children: snapshot
            .child_ids_slice(id)
            .iter()
            .filter_map(|id| copy(snapshot, *id))
            .collect(),
    })
}
impl TemplateNode {
    fn materialize(&self) -> NodeTree {
        let mut tree = if let Some(value) = &self.value {
            let mut parameter = authored_param(&self.label, value.clone());
            if let Some(constraints) = &self.constraints {
                parameter.constraints = constraints.clone();
            }
            NodeTree::new(parameter)
        } else {
            NodeTree::new(authored_folder(&self.label))
        };
        tree.node.node_data_mut().meta.decl_id = DeclId(self.id.clone());
        tree.node.node_data_mut().meta.short_name = self.short_name.clone();
        for child in &self.children {
            tree.push_child(child.materialize());
        }
        tree
    }
}
fn default_template(slot: usize, unpaged: bool) -> TemplateNode {
    let mut children = vec![
        field("Color", "color", ParamValue::Color(0.05, 0.05, 0.05, 1.0)),
        field("Text", "text", ParamValue::Str(String::new())),
        field("Image", "image", ParamValue::File(String::new())),
    ];
    if unpaged {
        children.push(field("Unpaged", "unpaged", ParamValue::Bool(false)));
    }
    TemplateNode {
        label: format!("Key {}", slot + 1),
        id: format!("key_{}", slot + 1),
        short_name: format!("key_{}", slot + 1),
        value: None,
        constraints: None,
        children,
    }
}
fn field(label: &str, id: &str, value: ParamValue) -> TemplateNode {
    TemplateNode {
        label: label.into(),
        id: id.into(),
        short_name: id.into(),
        value: Some(value),
        constraints: None,
        children: Vec::new(),
    }
}
pub(super) fn control_key(slot: usize, unpaged: bool) -> NodeTree {
    default_template(slot, unpaged).materialize()
}
pub(super) fn value_key(slot: usize) -> NodeTree {
    let mut parameter = authored_param(&format!("Key {}", slot + 1), ParamValue::Bool(false));
    parameter.node_data_mut().meta.decl_id = DeclId(format!("key_{}", slot + 1));
    parameter.node_data_mut().meta.short_name = format!("key_{}", slot + 1);
    NodeTree::new(parameter)
}
pub(super) fn authored_folder(label: &str) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}
pub(super) fn authored_param(label: &str, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}
pub(super) fn locked_folder(label: &str, id: &str) -> Folder {
    let mut folder = Folder::new(label);
    let data = folder.node_data_mut();
    data.meta.short_name = id.into();
    data.meta.decl_id = DeclId(id.into());
    data.meta.user_permissions = NodeUserPermissions::none();
    folder
}
pub(super) fn selector() -> Parameter {
    let mut parameter = authored_param("Active Page", ParamValue::Enum(paging::DEFAULT_PAGE_ID.into()));
    parameter.node_data_mut().meta.decl_id = DeclId(paging::ACTIVE_PAGE_PARAM.into());
    parameter.constraints.enum_options = vec![super::super::enum_option(paging::DEFAULT_PAGE_ID, "Default", 0)].into();
    parameter
}
pub(super) fn resize(
    snapshot: &ProcessTreeSnapshot,
    edits: &mut golden_core::edit::EditQueue,
    parent: NodeId,
    count: usize,
    control: bool,
    unpaged: bool,
) -> Result<(), String> {
    let children = snapshot.child_ids_slice(parent);
    if children.len() > 64 {
        return Err("Stream Deck key collection exceeds 64 children".into());
    }
    let existing: Vec<_> = children
        .iter()
        .copied()
        .filter(|id| {
            !control
                || snapshot
                    .node(*id)
                    .is_some_and(|node| node.param_value.is_none() && node.node_type != paging::PAGE_HOST_TYPE)
        })
        .collect();
    for slot in existing.len()..count {
        edits.push(Edit::AddNodeTree {
            parent,
            prev_sibling: None,
            tree: if control {
                control_key(slot, unpaged)
            } else {
                value_key(slot)
            },
        });
    }
    for id in existing.into_iter().skip(count) {
        edits.push(Edit::RemoveNode { node: id });
    }
    Ok(())
}
