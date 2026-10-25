use super::*;
use crate::engine::EngineEditError;
use crate::node::{DASHBOARD_WIDGET_ITEM_KIND, UserNodeRole};
use crate::parameter::CssValue;

impl<T: Node> Engine<T> {
    /// Search only declared layout children, never widget descendants or a full-tree snapshot.
    pub(super) fn ui_dashboard_layout_field(&self, owner: NodeId, field: &str) -> Option<NodeId> {
        let mut folder = self.nodes.get(owner)?.node_data().first_child;
        while let Some(id) = folder {
            let node = self.nodes.get(id)?;
            if node.node_data().meta.decl_id.0.rsplit('/').next() == Some("layout") {
                let mut child = node.node_data().first_child;
                while let Some(id) = child {
                    let node = self.nodes.get(id)?;
                    if node.node_data().meta.decl_id.0.rsplit('/').next() == Some(field) {
                        return Some(id);
                    }
                    child = node.node_data().next_sibling;
                }
                return None;
            }
            folder = node.node_data().next_sibling;
        }
        None
    }

    fn ui_dashboard_layout_kind(&self, parent: NodeId) -> Result<String, EngineEditError> {
        let target = self.nodes.get(parent);
        if !target.is_some_and(|node| {
            matches!(
                node.get_type(),
                DASHBOARD_PAGE_NODE_TYPE | DASHBOARD_WIDGET_CONTAINER_NODE_TYPE
            )
        }) {
            return Err(self.ui_dashboard_rejection(
                "DashboardLayout",
                parent,
                "destination is not a dashboard surface",
            ));
        }
        self.ui_dashboard_layout_field(parent, "layout_kind")
            .and_then(|id| self.nodes.get(id)?.engine_param_snapshot())
            .and_then(|param| param.value.as_enum())
            .ok_or_else(|| self.ui_dashboard_rejection("DashboardLayout", parent, "surface layout is unavailable"))
    }

    fn ui_dashboard_widget_parent(&self, widget: NodeId) -> Result<NodeId, EngineEditError> {
        self.nodes
            .get(widget)
            .filter(|node| {
                node.user_item_kind() == DASHBOARD_WIDGET_ITEM_KIND
                    && node.node_data().user_role == UserNodeRole::ItemRoot
            })
            .and_then(|node| node.node_data().parent)
            .ok_or_else(|| {
                self.ui_dashboard_rejection("DashboardLayout", widget, "target is not an attached dashboard widget")
            })
    }

    fn ui_dashboard_field_edit(&self, widget: NodeId, field: &str, value: ParamValue) -> Result<Edit, EngineEditError> {
        let id = self.ui_dashboard_layout_field(widget, field).ok_or_else(|| {
            self.ui_dashboard_rejection(
                "DashboardLayout",
                widget,
                format!("layout field '{field}' is unavailable"),
            )
        })?;
        if let Some(error) = self.read_only_ui_param_error(id) {
            return Err(error);
        }
        let behaviour = self
            .nodes
            .get(id)
            .and_then(Node::engine_param_snapshot)
            .ok_or_else(|| self.ui_dashboard_rejection("DashboardLayout", id, "layout field is not a parameter"))?
            .event_behaviour;
        let value = self.prepare_set_param_value(0, id, value)?;
        Ok(Edit::SetParam {
            node: id,
            value,
            behaviour,
        })
    }

    fn ui_dashboard_sizing_edit(&self, widget: NodeId, field: &str, enabled: bool) -> Result<Edit, EngineEditError> {
        let id = self
            .ui_dashboard_layout_field(widget, field)
            .ok_or_else(|| self.ui_dashboard_rejection("DashboardLayout", widget, "sizing control is unavailable"))?;
        if let Some(error) = self.read_only_ui_param_error(id) {
            return Err(error);
        }
        Ok(Edit::PatchMeta {
            node: id,
            patch: EngineNodeMetaPatch {
                enabled: Some(enabled),
                ..Default::default()
            },
        })
    }

    fn ui_dashboard_valid_size(&self, widget: NodeId, value: CssValue) -> Result<(), EngineEditError> {
        if !value.value.is_finite() || value.value < 0.0 {
            return Err(self.ui_dashboard_rejection("DashboardLayout", widget, "size must be finite and nonnegative"));
        }
        Ok(())
    }

    pub(super) fn ui_apply_resize_dashboard_widget(
        &mut self,
        widget: NodeId,
        width: Option<CssValue>,
        height: Option<CssValue>,
    ) -> Result<(), EngineEditError> {
        let parent = self.ui_dashboard_widget_parent(widget)?;
        let layout = self.ui_dashboard_layout_kind(parent)?;
        if (width.is_none() && height.is_none())
            || (width.is_some() && !matches!(layout.as_str(), "free" | "horizontal"))
            || (height.is_some() && !matches!(layout.as_str(), "free" | "vertical"))
        {
            return Err(self.ui_dashboard_rejection(
                "ResizeDashboardWidget",
                widget,
                "requested dimensions are not supported by the parent layout",
            ));
        }
        let mut edits = Vec::with_capacity(4);
        for (field, value) in [("width", width), ("height", height)] {
            if let Some(value) = value {
                self.ui_dashboard_valid_size(widget, value)?;
                edits.push(self.ui_dashboard_sizing_edit(widget, field, true)?);
                edits.push(self.ui_dashboard_field_edit(widget, field, ParamValue::CssValue(value))?);
            }
        }
        for edit in edits {
            self.edits.push(edit);
        }
        Ok(())
    }

    pub(super) fn ui_apply_move_dashboard_widget(
        &mut self,
        widget: NodeId,
        parent: NodeId,
        target_index: Option<usize>,
        placement: Option<UiDashboardWidgetPlacement>,
    ) -> Result<(), EngineEditError> {
        let previous_parent = self.ui_dashboard_widget_parent(widget)?;
        let layout = self.ui_dashboard_layout_kind(parent)?;
        let mut edits = Vec::with_capacity(7);
        if previous_parent != parent || target_index.is_some() {
            // Resolve a final user-item index against the authoritative order. Structural children
            // remain before the first item; excluded source identity cannot become its own sibling.
            let mut child = self.nodes.get(parent).and_then(|node| node.node_data().first_child);
            let mut previous = None;
            let mut items = 0;
            while let Some(id) = child {
                let node = self.nodes.get(id).expect("attached sibling must exist");
                child = node.node_data().next_sibling;
                if id == widget {
                    continue;
                }
                if node.node_data().user_role == UserNodeRole::ItemRoot {
                    if target_index == Some(items) {
                        break;
                    }
                    items += 1;
                }
                previous = Some(id);
            }
            self.edits.push(Edit::MoveNode {
                node: widget,
                new_parent: parent,
                new_prev_sibling: previous,
            });
            // Parent dependencies can retire and materialize sizing fields. Resolve placement
            // only after that lifecycle work, inside the same atomic/history transaction.
            self.settle_edit_notifications(16)?;
        }
        if let Some(placement) = placement {
            self.ui_dashboard_valid_size(widget, placement.width)?;
            self.ui_dashboard_valid_size(widget, placement.height)?;
            if !placement.position.0.is_finite() || !placement.position.1.is_finite() {
                return Err(self.ui_dashboard_rejection("MoveDashboardWidget", widget, "position must be finite"));
            }
            let enabled = placement.size_enabled.unwrap_or_default();
            for (field, value, enabled, supported) in [
                (
                    "width",
                    placement.width,
                    layout == "free" || enabled.width,
                    matches!(layout.as_str(), "free" | "horizontal"),
                ),
                (
                    "height",
                    placement.height,
                    layout == "free" || enabled.height,
                    matches!(layout.as_str(), "free" | "vertical"),
                ),
            ] {
                if supported {
                    edits.push(self.ui_dashboard_field_edit(widget, field, ParamValue::CssValue(value))?);
                    edits.push(self.ui_dashboard_sizing_edit(widget, field, enabled)?);
                }
            }
            if layout == "free" {
                edits.push(self.ui_dashboard_field_edit(widget, "anchor", ParamValue::Enum(placement.anchor))?);
                edits.push(self.ui_dashboard_field_edit(
                    widget,
                    "position",
                    ParamValue::Vec2(placement.position.0, placement.position.1),
                )?);
            }
        }
        for edit in edits {
            self.edits.push(edit);
        }
        Ok(())
    }
}
