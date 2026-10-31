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
                definition.transform.rotation = (f64::from(self.origin.rotation) + cross.atan2(dot)) as f32;
            }
            InstanceGeometryMode::Scale => {
                let (sin, cos) = f64::from(self.origin.rotation).sin_cos();
                let local = |point: [f64; 2]| [cos * point[0] + sin * point[1], -sin * point[0] + cos * point[1]];
                let from = local(start);
                let to = local(point);
                if from.into_iter().any(|axis| axis.abs() < 1e-6) {
                    return Err("scale pointer must be away from both local pivot axes".into());
                }
                definition.transform.scale = [
                    (f64::from(self.origin.scale[0]) * to[0] / from[0]) as f32,
                    (f64::from(self.origin.scale[1]) * to[1] / from[1]) as f32,
                ];
            }
        }
        definition.validate()
    }
}
