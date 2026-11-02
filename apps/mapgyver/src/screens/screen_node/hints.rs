use super::*;
use crate::screens::protocol::{DisplayIdentityHint, DisplayInfo};

impl ScreenNode {
    pub(super) fn is_display_hint_parameter(&self, id: NodeId) -> bool {
        [
            self.display_edid.id(),
            self.display_name.id(),
            self.display_os_index.id(),
            self.display_left.id(),
            self.display_top.id(),
            self.display_width.id(),
            self.display_height.id(),
        ]
        .contains(&id)
    }
    pub fn display_parameter_id(&self) -> NodeId {
        self.display.id()
    }
    pub fn target_parameter_id(&self) -> NodeId {
        self.target_kind.id()
    }
    pub fn authored_display_identity(&self) -> String {
        self.display.get_ref().as_str().to_owned()
    }

    /// Last explicitly selected display, projected from real hidden authored Parameters.
    /// Runtime matching must not rewrite these hints when a fallback display is used.
    pub fn display_hint(&self) -> DisplayIdentityHint {
        let width = self.display_width.get();
        let height = self.display_height.get();
        DisplayIdentityHint {
            identity: self.authored_display_identity(),
            edid: (!self.display_edid.get_ref().is_empty()).then(|| self.display_edid.get_ref().clone()),
            name: self.display_name.get_ref().clone(),
            index: self.display_os_index.get().max(0) as u32,
            bounds: (width > 0 && height > 0).then(|| PhysicalRectangle {
                left: self.display_left.get(),
                top: self.display_top.get(),
                extent: Extent2D::new(width as u32, height as u32).expect("positive hint extent"),
            }),
        }
    }

    pub fn display_hint_values(&self, display: &DisplayInfo) -> Vec<(NodeId, ParamValue)> {
        vec![
            (
                self.display_edid.id(),
                ParamValue::Str(display.edid.clone().unwrap_or_default()),
            ),
            (self.display_name.id(), ParamValue::Str(display.name.clone())),
            (
                self.display_os_index.id(),
                ParamValue::Int(i32::try_from(display.index).unwrap_or(0)),
            ),
            (self.display_left.id(), ParamValue::Int(display.bounds.left)),
            (self.display_top.id(), ParamValue::Int(display.bounds.top)),
            (
                self.display_width.id(),
                ParamValue::Int(i32::try_from(display.bounds.extent.width()).unwrap_or(0)),
            ),
            (
                self.display_height.id(),
                ParamValue::Int(i32::try_from(display.bounds.extent.height()).unwrap_or(0)),
            ),
        ]
    }

    pub fn display_selection_values(&self, display: &DisplayInfo) -> Vec<(NodeId, ParamValue)> {
        let mut values = vec![(self.display.id(), ParamValue::Enum(display.id.clone()))];
        values.extend(self.display_hint_values(display));
        values
    }

    pub(crate) fn remember_display(&mut self, display: &DisplayInfo) {
        self.display_edid = ParameterHandle::new(display.edid.clone().unwrap_or_default());
        self.display_name = ParameterHandle::new(display.name.clone());
        self.display_os_index = ParameterHandle::new(i32::try_from(display.index).unwrap_or(0));
        self.display_left = ParameterHandle::new(display.bounds.left);
        self.display_top = ParameterHandle::new(display.bounds.top);
        self.display_width = ParameterHandle::new(i32::try_from(display.bounds.extent.width()).unwrap_or(0));
        self.display_height = ParameterHandle::new(i32::try_from(display.bounds.extent.height()).unwrap_or(0));
    }

    /// A swap transfers the saved evidence exactly, including an offline display's hints.
    pub(super) fn copy_display_hint_values(&self, source: &Self) -> Vec<(NodeId, ParamValue)> {
        vec![
            (
                self.display_edid.id(),
                ParamValue::Str(source.display_edid.get_ref().clone()),
            ),
            (
                self.display_name.id(),
                ParamValue::Str(source.display_name.get_ref().clone()),
            ),
            (
                self.display_os_index.id(),
                ParamValue::Int(source.display_os_index.get()),
            ),
            (self.display_left.id(), ParamValue::Int(source.display_left.get())),
            (self.display_top.id(), ParamValue::Int(source.display_top.get())),
            (self.display_width.id(), ParamValue::Int(source.display_width.get())),
            (self.display_height.id(), ParamValue::Int(source.display_height.get())),
        ]
    }

    pub(super) fn validate_display_hint(&self, change: Option<&ParameterValidationContext<'_>>) -> Result<(), String> {
        validate_endpoint(&read(&self.display_edid, change), 256, "display EDID hint")?;
        validate_endpoint(&read(&self.display_name, change), 1024, "display name hint")?;
        if read(&self.display_os_index, change) < 0
            || read(&self.display_width, change) < 0
            || read(&self.display_height, change) < 0
        {
            return Err("display recovery index and extents must be nonnegative".into());
        }
        Ok(())
    }
}
