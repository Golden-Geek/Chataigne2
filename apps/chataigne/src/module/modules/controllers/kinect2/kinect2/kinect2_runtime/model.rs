use super::super::{KINECT2_SPACE_HEAD, KINECT2_SPACE_TORSO};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct KinectVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl KinectVec3 {
    pub(crate) const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub(crate) const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn subtract(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub(crate) fn midpoint(self, other: Self) -> Self {
        Self::new(
            (self.x + other.x) * 0.5,
            (self.y + other.y) * 0.5,
            (self.z + other.z) * 0.5,
        )
    }

    pub(crate) fn scale(self, value: f64) -> Self {
        Self::new(self.x * value, self.y * value, self.z * value)
    }

    pub(crate) fn distance(self, other: Self) -> f64 {
        let delta = self.subtract(other);
        (delta.x * delta.x + delta.y * delta.y + delta.z * delta.z).sqrt()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum KinectTrackingState {
    #[default]
    NotTracked,
    Inferred,
    Tracked,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct KinectJointSample {
    pub position: KinectVec3,
    pub tracking_state: KinectTrackingState,
}

impl KinectJointSample {
    pub(crate) fn available_position(self) -> KinectVec3 {
        if self.tracking_state == KinectTrackingState::NotTracked {
            KinectVec3::ZERO
        } else {
            self.position
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum KinectJoint {
    SpineBase,
    SpineMid,
    Neck,
    Head,
    ShoulderLeft,
    ElbowLeft,
    WristLeft,
    HandLeft,
    ShoulderRight,
    ElbowRight,
    WristRight,
    HandRight,
    HipLeft,
    KneeLeft,
    AnkleLeft,
    FootLeft,
    HipRight,
    KneeRight,
    AnkleRight,
    FootRight,
    SpineShoulder,
    HandTipLeft,
    ThumbLeft,
    HandTipRight,
    ThumbRight,
}

impl KinectJoint {
    pub(crate) const COUNT: usize = 25;

    pub(crate) const ALL: [Self; Self::COUNT] = [
        Self::SpineBase,
        Self::SpineMid,
        Self::Neck,
        Self::Head,
        Self::ShoulderLeft,
        Self::ElbowLeft,
        Self::WristLeft,
        Self::HandLeft,
        Self::ShoulderRight,
        Self::ElbowRight,
        Self::WristRight,
        Self::HandRight,
        Self::HipLeft,
        Self::KneeLeft,
        Self::AnkleLeft,
        Self::FootLeft,
        Self::HipRight,
        Self::KneeRight,
        Self::AnkleRight,
        Self::FootRight,
        Self::SpineShoulder,
        Self::HandTipLeft,
        Self::ThumbLeft,
        Self::HandTipRight,
        Self::ThumbRight,
    ];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }

    #[cfg(windows)]
    pub(super) fn from_raw(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::SpineBase),
            1 => Some(Self::SpineMid),
            2 => Some(Self::Neck),
            3 => Some(Self::Head),
            4 => Some(Self::ShoulderLeft),
            5 => Some(Self::ElbowLeft),
            6 => Some(Self::WristLeft),
            7 => Some(Self::HandLeft),
            8 => Some(Self::ShoulderRight),
            9 => Some(Self::ElbowRight),
            10 => Some(Self::WristRight),
            11 => Some(Self::HandRight),
            12 => Some(Self::HipLeft),
            13 => Some(Self::KneeLeft),
            14 => Some(Self::AnkleLeft),
            15 => Some(Self::FootLeft),
            16 => Some(Self::HipRight),
            17 => Some(Self::KneeRight),
            18 => Some(Self::AnkleRight),
            19 => Some(Self::FootRight),
            20 => Some(Self::SpineShoulder),
            21 => Some(Self::HandTipLeft),
            22 => Some(Self::ThumbLeft),
            23 => Some(Self::HandTipRight),
            24 => Some(Self::ThumbRight),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KinectBodySnapshot {
    pub tracking_id: u64,
    pub joints: [KinectJointSample; KinectJoint::COUNT],
}

impl KinectBodySnapshot {
    pub(crate) fn joint(&self, joint: KinectJoint) -> KinectJointSample {
        self.joints[joint.index()]
    }

    pub(crate) fn reference_depth(&self) -> f64 {
        for joint in [
            KinectJoint::SpineMid,
            KinectJoint::SpineBase,
            KinectJoint::SpineShoulder,
            KinectJoint::Head,
        ] {
            let position = self.joint(joint).available_position();
            if position.z.abs() > f64::EPSILON {
                return position.z;
            }
        }

        f64::INFINITY
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KinectFrameSnapshot {
    pub sensor_available: bool,
    pub timestamp_100ns: u64,
    pub tracked_bodies: Vec<KinectBodySnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KinectRuntimePoll {
    pub sensor_available: bool,
    pub frame: Option<KinectFrameSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KinectReferenceSpace {
    Absolute,
    Torso,
    Head,
}

impl KinectReferenceSpace {
    pub(crate) fn from_variant(value: &str) -> Self {
        match value.trim() {
            KINECT2_SPACE_TORSO => Self::Torso,
            KINECT2_SPACE_HEAD => Self::Head,
            _ => Self::Absolute,
        }
    }
}
