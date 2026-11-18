//! Serialize only the requested subtree from a coherent, borrowed Golden projection.
use super::values;
use golden_engine::ui_read_model::{UiReadModel, UiSnapshotCapture};
use golden_protocol::{UiCreatedNode, UiNodeDataDto, UiNodeDto, UiOscAddressRequest, UiParamDto};
use serde::{
    Serialize, Serializer,
    ser::{Error as _, SerializeMap},
};
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::Write as _,
    io::{self, Write},
};

const MAX_DEPTH: usize = 256;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

pub(in crate::ui_server) fn osc_address(model: &UiReadModel, request: UiOscAddressRequest) -> Option<String> {
    let (snapshot, ancestry) = model.capture_node_ancestry(request.node, MAX_DEPTH)?;
    if snapshot.project_generation().get() != request.project_generation {
        return None;
    }
    let mut address = String::new();
    for edge in ancestry.windows(2) {
        let parent = snapshot.node(edge[0])?;
        let child = snapshot.node(edge[1])?;
        let name = segment(child, |name| unique_sibling_name(&snapshot, parent, name));
        write!(address, "/{name}").ok()?;
    }
    if address.is_empty() {
        address.push('/');
    }
    Some(address)
}

#[derive(Debug)]
pub(super) enum QueryError {
    InvalidAttribute,
    MissingPath,
    TooLarge,
}

impl QueryError {
    pub fn status(&self) -> &'static str {
        match self {
            Self::InvalidAttribute => "400 Bad Request",
            Self::MissingPath => "404 Not Found",
            Self::TooLarge => "413 Payload Too Large",
        }
    }
    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidAttribute => "Unknown OSCQuery attribute",
            Self::MissingPath => "Unknown OSC address",
            Self::TooLarge => "The OSCQuery response exceeds its limits; request a smaller subtree",
        }
    }
}

pub(super) fn describe(
    snapshot: &UiSnapshotCapture,
    path: &str,
    query: Option<&str>,
) -> Result<Option<Vec<u8>>, QueryError> {
    let selected = query
        .filter(|query| !query.is_empty())
        .map(|query| query.split('&').collect::<Vec<_>>());
    if selected.as_ref().is_some_and(|attributes| {
        attributes.iter().any(|name| {
            !matches!(
                *name,
                "FULL_PATH"
                    | "DESCRIPTION"
                    | "CONTENTS"
                    | "ACCESS"
                    | "TYPE"
                    | "VALUE"
                    | "RANGE"
                    | "GOLDEN_META"
                    | "GOLDEN_ADD_TYPES"
            )
        })
    }) {
        return Err(QueryError::InvalidAttribute);
    }
    let node = resolve(snapshot, path).ok_or(QueryError::MissingPath)?;
    let document = Document {
        snapshot,
        node,
        path,
        selected: selected.as_deref(),
        depth: 0,
    };
    if selected.as_ref().is_some_and(|attrs| attrs.as_slice() == ["VALUE"])
        && !parameter(node).is_some_and(|param| values::readable(&param.value))
    {
        return Ok(None);
    }
    let mut bytes = Limit(Vec::new());
    serde_json::to_writer(&mut bytes, &document).map_err(|_| QueryError::TooLarge)?;
    Ok(Some(bytes.0))
}

pub(super) fn resolve<'a>(snapshot: &'a UiSnapshotCapture, path: &str) -> Option<&'a UiNodeDto> {
    let mut node = snapshot.node(snapshot.root())?;
    if path != "/" {
        for segment in path.strip_prefix('/')?.split('/') {
            node = resolve_child(snapshot, node, segment)?;
        }
    }
    Some(node)
}

fn resolve_child<'a>(snapshot: &'a UiSnapshotCapture, parent: &UiNodeDto, segment: &str) -> Option<&'a UiNodeDto> {
    let children = || parent.children.iter().filter_map(|id| snapshot.node(*id));
    if let Some(uuid) = segment.strip_prefix('~') {
        let child = children().find(|child| child.uuid.0.as_simple().encode_lower(&mut [0; 32]) == uuid)?;
        // Accept the address returned by the hierarchy, including its fallback rule.
        if short_name(child).is_none_or(|name| !unique_sibling_name(snapshot, parent, name)) {
            return Some(child);
        }
    } else {
        let mut matches = children().filter(|child| short_name(child) == Some(segment));
        let child = matches.next()?;
        if matches.next().is_none() {
            return Some(child);
        }
    }
    None
}

fn unique_sibling_name(snapshot: &UiSnapshotCapture, parent: &UiNodeDto, name: &str) -> bool {
    parent
        .children
        .iter()
        .filter_map(|id| snapshot.node(*id))
        .filter(|child| short_name(child) == Some(name))
        .take(2)
        .count()
        == 1
}

pub(super) fn parameter(node: &UiNodeDto) -> Option<&UiParamDto> {
    match &node.data {
        UiNodeDataDto::Parameter { param } => Some(param),
        _ => None,
    }
}

pub(super) fn child_path(
    snapshot: &UiSnapshotCapture,
    parent: &UiNodeDto,
    path: &str,
    created: &UiCreatedNode,
) -> Option<String> {
    let (name, _) = named_children(snapshot, parent)
        .find(|(_, child)| child.node_id == created.node_id && child.uuid == created.uuid)?;
    Some(format!("{}/{name}", path.trim_end_matches('/')))
}

fn short_name(node: &UiNodeDto) -> Option<&str> {
    let name = node.meta.short_name.as_str();
    (!name.is_empty()
        && name.len() <= 120
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte)))
    .then_some(name)
}

fn segment(node: &UiNodeDto, is_unique: impl FnOnce(&str) -> bool) -> Cow<'_, str> {
    if let Some(name) = short_name(node)
        && is_unique(name)
    {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("~{}", node.uuid.0.as_simple()))
    }
}

fn named_children<'a>(
    snapshot: &'a UiSnapshotCapture,
    node: &'a UiNodeDto,
) -> impl Iterator<Item = (Cow<'a, str>, &'a UiNodeDto)> {
    let children = || node.children.iter().filter_map(|id| snapshot.node(*id));
    let mut counts = HashMap::new();
    for name in children().filter_map(short_name) {
        *counts.entry(name).or_insert(0usize) += 1;
    }
    children().map(move |child| (segment(child, |name| counts.get(name) == Some(&1)), child))
}

struct Document<'a> {
    snapshot: &'a UiSnapshotCapture,
    node: &'a UiNodeDto,
    path: &'a str,
    selected: Option<&'a [&'a str]>,
    depth: usize,
}

impl Document<'_> {
    fn wants(&self, attribute: &str) -> bool {
        self.selected.is_none_or(|attributes| attributes.contains(&attribute))
    }
    fn description(&self) -> &str {
        let meta = &self.node.meta;
        meta.description
            .as_deref()
            .or_else(|| {
                let key = meta.declared_description_key.as_ref()?;
                self.snapshot
                    .schema()
                    .declared_descriptions
                    .iter()
                    .find(|descriptor| &descriptor.key == key)
                    .map(|entry| entry.description.as_str())
            })
            .unwrap_or(&meta.label)
    }
}

impl Serialize for Document<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.depth > MAX_DEPTH {
            return Err(S::Error::custom("OSCQuery subtree is too deep"));
        }
        let mut map = serializer.serialize_map(None)?;
        let param = parameter(self.node);
        if self.wants("FULL_PATH") {
            map.serialize_entry("FULL_PATH", self.path)?;
        }
        if self.wants("DESCRIPTION") {
            map.serialize_entry("DESCRIPTION", self.description())?;
        }
        if self.selected.is_some() && self.wants("GOLDEN_META") {
            let meta = &self.node.meta;
            map.serialize_entry(
                "GOLDEN_META",
                &json!({
                    "label": meta.label,
                    "enabled": meta.enabled,
                    "canBeDisabled": meta.can_be_disabled,
                    "description": meta.description,
                    "tags": meta.tags,
                    "color": meta.presentation.color,
                    "collapsed": meta.presentation.collapsed,
                    "permissions": meta.user_permissions,
                }),
            )?;
        }
        if self.wants("GOLDEN_ADD_TYPES") && !self.node.creatable_user_items.is_empty() {
            let items = self
                .node
                .creatable_user_items
                .iter()
                .map(|item| json!({"type": item.node_type, "label": item.label}))
                .collect::<Vec<Value>>();
            map.serialize_entry("GOLDEN_ADD_TYPES", &items)?;
        }
        if self.wants("ACCESS") {
            let access = param.map_or(0, |param| {
                u8::from(values::readable(&param.value)) | (u8::from(!param.read_only) << 1)
            });
            map.serialize_entry("ACCESS", &access)?;
        }
        if let Some(param) = param {
            if self.wants("TYPE") {
                map.serialize_entry("TYPE", values::type_tag(&param.value))?;
            }
            if self.wants("VALUE")
                && let Some(value) = values::value(param)
            {
                map.serialize_entry("VALUE", &value)?;
            }
            if self.wants("RANGE")
                && let Some(range) = values::range(self.snapshot.schema(), param)
            {
                map.serialize_entry("RANGE", &range)?;
            }
        }
        if self.wants("CONTENTS") && (param.is_none() || !self.node.children.is_empty()) {
            map.serialize_entry("CONTENTS", &Children(self))?;
        }
        map.end()
    }
}

struct Children<'a>(&'a Document<'a>);

impl Serialize for Children<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let parent = self.0;
        let mut map = serializer.serialize_map(None)?;
        for (name, child) in named_children(parent.snapshot, parent.node) {
            let path = format!("{}/{name}", parent.path.trim_end_matches('/'));
            map.serialize_entry(
                &name,
                &Document {
                    snapshot: parent.snapshot,
                    node: child,
                    path: &path,
                    selected: None,
                    depth: parent.depth + 1,
                },
            )?;
        }
        map.end()
    }
}

struct Limit(Vec<u8>);

impl Write for Limit {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RESPONSE_BYTES - self.0.len() {
            return Err(io::Error::other("OSCQuery response is too large"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
