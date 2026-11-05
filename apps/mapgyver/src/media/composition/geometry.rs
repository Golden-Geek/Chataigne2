use super::protocol::{InstanceDefinition, InstanceGeometryEdit, InstanceGeometryMode};

impl InstanceGeometryEdit {
    /// Change only the requested transform field on a validated clone of the current definition.
    pub(super) fn apply(self, definition: &mut InstanceDefinition) -> Result<(), String> {
        InstanceDefinition {
            transform: self.origin,
            ..InstanceDefinition::default()
        }
        .validate()?;
        if ![self.start.x, self.start.y, self.point.x, self.point.y]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err("geometry pointer coordinates must be finite".into());
        }
        let position = self.origin.position.map(f64::from);
        let start = [
            f64::from(self.start.x) - position[0],
            f64::from(self.start.y) - position[1],
        ];
        let point = [
            f64::from(self.point.x) - position[0],
            f64::from(self.point.y) - position[1],
        ];
        match self.mode {
            InstanceGeometryMode::Translate => {
                definition.transform.position = [
                    (position[0] + f64::from(self.point.x) - f64::from(self.start.x)) as f32,
                    (position[1] + f64::from(self.point.y) - f64::from(self.start.y)) as f32,
                ];
            }
            InstanceGeometryMode::Rotate => {
                if start == [0.0; 2] || point == [0.0; 2] {
                    return Err("rotation pointer must be away from the pivot".into());
                }
                let cross = start[0] * point[1] - start[1] * point[0];
                let dot = start[0] * point[0] + start[1] * point[1];
                definition.transform.rotation_degrees =
                    (f64::from(self.origin.rotation_degrees) + cross.atan2(dot).to_degrees()) as f32;
            }
            InstanceGeometryMode::Resize { handle, extent } => {
                if definition.transform.rotation_degrees != self.origin.rotation_degrees
                    || definition.transform.pivot != self.origin.pivot
                {
                    return Err("resize basis changed; start a new gesture".into());
                }
                let (sin, cos) = f64::from(self.origin.rotation_degrees).to_radians().sin_cos();
                let delta = [point[0] - start[0], point[1] - start[1]];
                let local = [cos * delta[0] + sin * delta[1], -sin * delta[0] + cos * delta[1]];
                let handle = handle.position();
                let dimensions = [f64::from(extent.width()), f64::from(extent.height())];
                let mut shift = [0.; 2];
                for axis in 0..2 {
                    if handle[axis] == 0.5 {
                        continue;
                    }
                    let size_delta = local[axis] * (2. * handle[axis] - 1.);
                    definition.transform.scale[axis] =
                        (f64::from(self.origin.scale[axis]) + size_delta / dimensions[axis]) as f32;
                    shift[axis] = -(1. - handle[axis] - f64::from(self.origin.pivot[axis])) * size_delta;
                }
                definition.transform.position = [
                    (position[0] + cos * shift[0] - sin * shift[1]) as f32,
                    (position[1] + sin * shift[0] + cos * shift[1]) as f32,
                ];
            }
        }
        definition.validate()
    }
}
