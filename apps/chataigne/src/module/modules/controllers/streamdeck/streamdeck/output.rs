use super::*;
use golden_io::{RetainedPayload, RevisionSource};

pub(super) struct Output {
    pub(super) dirty: bool,
    draw: RevisionSource,
    draw_revision: u64,
    selection: RevisionSource,
    selection_revision: u64,
    selected: String,
}
impl Default for Output {
    fn default() -> Self {
        Self {
            dirty: true,
            draw: RevisionSource::default(),
            draw_revision: 0,
            selection: RevisionSource::default(),
            selection_revision: 0,
            selected: String::new(),
        }
    }
}
impl Output {
    pub(super) fn invalidate(&mut self) {
        self.dirty = true;
        self.draw_revision = self.draw.advance();
    }
    pub(super) fn cancel(&mut self) {
        self.invalidate();
        self.selection_revision = self.selection.advance();
    }
}
pub(super) fn selection(value: &str) -> &str {
    if value.is_empty() {
        AUTO_DEVICE_VARIANT
    } else {
        value
    }
}
impl StreamDeckModule {
    pub(super) fn capture_frame(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<RetainedPayload<runtime::Frame>, String> {
        let requested = self.device.get_ref();
        let requested = selection(requested.as_str());
        if requested.len() > runtime::MAX_SERIAL {
            return Err("Stream Deck serial exceeds 256 bytes".into());
        }
        let reservation = runtime::frames()?
            .try_reserve(runtime::FRAME_WEIGHT)
            .map_err(|_| "Stream Deck feedback lifetime capacity is full")?;
        let parameters = self.base.parameters_id().ok_or("Stream Deck parameters unavailable")?;
        let default = snapshot
            .find_child_by_decl_id(parameters, KEYS_FOLDER)
            .ok_or("Stream Deck default layout unavailable")?;
        let page = self.layout.active(snapshot, parameters)?;
        let root = self
            .layout
            .control_page(snapshot, parameters, &page)
            .ok_or("Stream Deck control page unavailable")?;
        let values = self
            .layout
            .value_page(
                snapshot,
                self.base.values_id().ok_or("Stream Deck values unavailable")?,
                &page,
            )
            .ok_or("Stream Deck input page unavailable")?;
        let default_keys = keys(snapshot, default)?;
        let active_keys = keys(snapshot, root)?;
        let count = self.model().key_count();
        let mut visuals = Vec::with_capacity(count);
        for slot in 0..count {
            let default = default_keys.get(slot).copied();
            if default.is_some_and(|id| snapshot.child_ids_slice(id).len() > 16) {
                return Err("Stream Deck control key exceeds 16 fields".into());
            }
            let unpaged = default
                .and_then(|id| field(snapshot, id, FIELD_UNPAGED))
                .and_then(ParamValue::as_bool)
                .unwrap_or(false);
            let source = if unpaged {
                default
            } else {
                active_keys.get(slot).copied()
            };
            visuals.push(match source {
                Some(source) => visual(snapshot, source)?,
                None => runtime::KeyVisual::default(),
            });
        }
        let routes = (0..runtime::MAX_KEYS)
            .map(|index| input::Route::capture(snapshot, self.id(), &page, values, index, count))
            .collect::<Result<Vec<_>, _>>()?;
        if self.output.selected != requested {
            self.output.selection_revision = self.output.selection.advance();
            self.output.selected = requested.into();
        }
        reservation
            .retain(
                runtime::Frame {
                    selection: requested.into(),
                    selection_guard: self.output.selection.reader().guard_at(self.output.selection_revision),
                    draw_guard: self.output.draw.reader().guard_at(self.output.draw_revision),
                    brightness: (self.brightness.get() * 100.0).clamp(0.0, 100.0) as u8,
                    visuals,
                    routes,
                },
                runtime::FRAME_WEIGHT,
            )
            .map_err(|e| e.to_string())
    }
}
fn keys(snapshot: &ProcessTreeSnapshot, parent: NodeId) -> Result<Vec<NodeId>, String> {
    let children = snapshot.child_ids_slice(parent);
    if children.len() > runtime::MAX_KEYS {
        return Err("Stream Deck control collection exceeds 64 children".into());
    }
    Ok(children
        .iter()
        .copied()
        .filter(|id| {
            snapshot
                .node(*id)
                .is_some_and(|node| node.param_value.is_none() && node.node_type != paging::PAGE_HOST_TYPE)
        })
        .collect())
}
fn field(snapshot: &ProcessTreeSnapshot, parent: NodeId, index: usize) -> Option<&ParamValue> {
    // Positional primitives preserve the existing authored control contract.
    snapshot
        .child_ids_slice(parent)
        .iter()
        .filter_map(|id| snapshot.node(*id)?.param_value.as_ref())
        .nth(index)
}
fn visual(snapshot: &ProcessTreeSnapshot, parent: NodeId) -> Result<runtime::KeyVisual, String> {
    if snapshot.child_ids_slice(parent).len() > 16 {
        return Err("Stream Deck control key exceeds 16 fields".into());
    }
    let color = field(snapshot, parent, FIELD_COLOR)
        .and_then(ParamValue::as_color)
        .unwrap_or((0.0, 0.0, 0.0, 1.0));
    let text = bounded_string(field(snapshot, parent, FIELD_TEXT), runtime::MAX_TEXT)?;
    let image = bounded_string(field(snapshot, parent, FIELD_IMAGE), runtime::MAX_IMAGE_PATH)?;
    let visual = runtime::KeyVisual { color, text, image };
    visual.validate()?;
    Ok(visual)
}
fn bounded_string(value: Option<&ParamValue>, limit: usize) -> Result<String, String> {
    let value = match value {
        Some(ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value)) => value.as_str(),
        _ => "",
    };
    if value.len() > limit {
        return Err(format!("Stream Deck visual string exceeds {limit} bytes"));
    }
    Ok(value.into())
}
