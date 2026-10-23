use super::super::KINECT2_HAND_SPEED_RESET_SECS;
use super::*;

#[derive(Clone, Debug)]
pub(in super::super) struct Prepared {
    pub(in super::super) space: KinectReferenceSpace,
    pub(in super::super) sensor_available: bool,
    pub(in super::super) tracked_bodies: i32,
    pub(in super::super) tracking_id: String,
    pub(in super::super) joints: [KinectVec3; KinectJoint::COUNT],
    pub(in super::super) distance: f64,
    pub(in super::super) rotation: (f64, f64),
    pub(in super::super) speed: KinectVec3,
    pub(in super::super) last_event: Option<String>,
    pub(in super::super) incoming: bool,
}
impl Prepared {
    pub(in super::super) fn empty(space: KinectReferenceSpace) -> Self {
        Self {
            space,
            sensor_available: false,
            tracked_bodies: 0,
            tracking_id: String::new(),
            joints: [KinectVec3::ZERO; KinectJoint::COUNT],
            distance: 0.0,
            rotation: (0.0, 0.0),
            speed: KinectVec3::ZERO,
            last_event: None,
            incoming: false,
        }
    }
}
struct History {
    tracking: u64,
    space: KinectReferenceSpace,
    timestamp: u64,
    midpoint: KinectVec3,
}
pub(super) struct Preparer {
    current: Prepared,
    selected: Option<u64>,
    history: Option<History>,
}
impl Preparer {
    pub(super) fn new() -> Self {
        Self {
            current: Prepared::empty(KinectReferenceSpace::Absolute),
            selected: None,
            history: None,
        }
    }
    pub(super) fn reset_history(&mut self) {
        self.history = None;
    }
    fn clear_values(&mut self, space: KinectReferenceSpace) {
        // Tracking status is snapshot state; a cancelled predecessor cannot consume it.
        let last_event = self.current.last_event.take();
        self.current = Prepared::empty(space);
        self.current.last_event = last_event;
    }
    pub(super) fn prepare(&mut self, poll: KinectRuntimePoll, space: KinectReferenceSpace) -> Result<Prepared, String> {
        validate(&poll)?;
        if self.current.space != space {
            self.reset_history();
        }
        self.current.space = space;
        self.current.incoming = false;
        if !poll.sensor_available {
            self.clear_values(space);
            self.lost();
            return Ok(self.current.clone());
        }
        self.current.sensor_available = true;
        let Some(frame) = poll.frame else {
            return Ok(self.current.clone());
        };
        self.current.tracked_bodies = frame.tracked_bodies.len() as i32;
        let Some(body) = select_primary_body(&frame.tracked_bodies) else {
            self.clear_values(space);
            self.current.sensor_available = true;
            self.lost();
            return Ok(self.current.clone());
        };
        let origin = reference_origin(body, space);
        let joints = KinectJoint::ALL.map(|joint| body.joint(joint).available_position().subtract(origin));
        let left = joints[KinectJoint::HandLeft.index()];
        let right = joints[KinectJoint::HandRight.index()];
        let distance = left.distance(right);
        let rotation = planar_direction(left, right);
        let midpoint = left.midpoint(right);
        let speed = self.speed(body.tracking_id, space, frame.timestamp_100ns, midpoint);
        if !joints.iter().copied().all(finite)
            || !finite(speed)
            || !distance.is_finite()
            || !rotation.0.is_finite()
            || !rotation.1.is_finite()
        {
            return Err("Kinect derived skeleton is not finite".into());
        }
        // Preserve the module's existing noise threshold on published state, not raw history.
        for (index, value) in joints.into_iter().enumerate() {
            if vec_changed(self.current.joints[index], value) {
                self.current.joints[index] = value;
            }
        }
        if changed(self.current.distance, distance) {
            self.current.distance = distance;
        }
        if changed(self.current.rotation.0, rotation.0) || changed(self.current.rotation.1, rotation.1) {
            self.current.rotation = rotation;
        }
        if vec_changed(self.current.speed, speed) {
            self.current.speed = speed;
        }
        self.current.tracking_id = body.tracking_id.to_string();
        self.current.incoming = true;
        if self.selected != Some(body.tracking_id) {
            self.current.last_event = Some(format!("Tracking body {}", body.tracking_id));
        }
        self.selected = Some(body.tracking_id);
        Ok(self.current.clone())
    }
    fn lost(&mut self) {
        if self.selected.take().is_some() {
            self.current.last_event = Some("Lost tracked body".into());
        }
        self.reset_history();
    }
    fn speed(
        &mut self,
        tracking: u64,
        space: KinectReferenceSpace,
        timestamp: u64,
        midpoint: KinectVec3,
    ) -> KinectVec3 {
        let previous = self.history.replace(History {
            tracking,
            space,
            timestamp,
            midpoint,
        });
        let Some(previous) = previous else {
            return KinectVec3::ZERO;
        };
        if previous.tracking != tracking || previous.space != space || timestamp <= previous.timestamp {
            return KinectVec3::ZERO;
        }
        let delta = (timestamp - previous.timestamp) as f64 / 10_000_000.0;
        if delta <= f64::EPSILON || delta > KINECT2_HAND_SPEED_RESET_SECS {
            return KinectVec3::ZERO;
        }
        midpoint.subtract(previous.midpoint).scale(1.0 / delta)
    }
}
pub(super) fn validate(row: &KinectRuntimePoll) -> Result<(), String> {
    let Some(frame) = &row.frame else {
        return Ok(());
    };
    if frame.sensor_available != row.sensor_available || frame.tracked_bodies.capacity() > 6 {
        return Err("Kinect frame exceeds six bodies or has inconsistent availability".into());
    }
    for (i, body) in frame.tracked_bodies.iter().enumerate() {
        if frame.tracked_bodies[..i]
            .iter()
            .any(|b| b.tracking_id == body.tracking_id)
            || !body.joints.iter().all(|joint| finite(joint.position))
        {
            return Err("Kinect frame has duplicate identities or nonfinite joints".into());
        }
    }
    Ok(())
}
fn finite(v: KinectVec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}
fn changed(a: f64, b: f64) -> bool {
    (a - b).abs() > 0.000_001
}
fn vec_changed(a: KinectVec3, b: KinectVec3) -> bool {
    changed(a.x, b.x) || changed(a.y, b.y) || changed(a.z, b.z)
}
fn select_primary_body(bodies: &[KinectBodySnapshot]) -> Option<&KinectBodySnapshot> {
    bodies
        .iter()
        .min_by(|a, b| a.reference_depth().total_cmp(&b.reference_depth()))
}
fn reference_origin(body: &KinectBodySnapshot, space: KinectReferenceSpace) -> KinectVec3 {
    match space {
        KinectReferenceSpace::Absolute => KinectVec3::ZERO,
        KinectReferenceSpace::Torso => joint_or_zero(
            body,
            &[
                KinectJoint::SpineMid,
                KinectJoint::SpineBase,
                KinectJoint::SpineShoulder,
            ],
        ),
        KinectReferenceSpace::Head => joint_or_zero(body, &[KinectJoint::Head]),
    }
}
fn joint_or_zero(body: &KinectBodySnapshot, joints: &[KinectJoint]) -> KinectVec3 {
    joints
        .iter()
        .map(|j| body.joint(*j).available_position())
        .find(|p| p.x.abs() > f64::EPSILON || p.y.abs() > f64::EPSILON || p.z.abs() > f64::EPSILON)
        .unwrap_or(KinectVec3::ZERO)
}
fn planar_direction(left: KinectVec3, right: KinectVec3) -> (f64, f64) {
    let delta = right.subtract(left);
    let length = (delta.x * delta.x + delta.y * delta.y).sqrt();
    if length <= f64::EPSILON {
        (0.0, 0.0)
    } else {
        (delta.x / length, delta.y / length)
    }
}
#[cfg(test)]
mod tests;
