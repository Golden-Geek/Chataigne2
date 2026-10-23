use super::*;

pub(in super::super) struct Prepared {
    pub(in super::super) poll: UltraleapRuntimePoll,
    pub(in super::super) distance: f64,
}
#[derive(Default)]
pub(super) struct Preparer {
    tracking: bool,
}
impl Preparer {
    pub(super) fn prepare(&mut self, mut poll: UltraleapRuntimePoll) -> Result<Prepared, String> {
        validate(&poll)?;
        if !poll.service_connected {
            poll.connected_devices = 0;
        }
        let mut distance = 0.0;
        if !poll.service_connected || poll.connected_devices == 0 {
            self.tracking = false;
            poll.frame = None;
        } else if let Some(frame) = &poll.frame {
            if frame.left.active && frame.right.active {
                distance = frame.left.palm_position.distance(frame.right.palm_position);
                if !distance.is_finite() {
                    return Err("Ultraleap hands distance is not finite".into());
                }
            }
            let active = frame.hand_count > 0;
            if active && !self.tracking {
                let suffix = if frame.hand_count == 1 { "" } else { "s" };
                poll.last_event = Some(format!("Tracking {} hand{suffix}", frame.hand_count));
            } else if !active && self.tracking {
                poll.last_event = Some("Lost Ultraleap tracking".into());
            }
            self.tracking = active;
        }
        Ok(Prepared { poll, distance })
    }
}

pub(super) fn coalesce(rows: Vec<UltraleapRuntimePoll>) -> Result<Vec<UltraleapRuntimePoll>, String> {
    if rows.capacity() > 16 {
        return Err("Ultraleap provider turn exceeds 16 records".into());
    }
    let mut result: Vec<UltraleapRuntimePoll> = Vec::with_capacity(rows.len());
    for row in rows {
        validate(&row)?;
        let replace = result.last().is_some_and(|previous| {
            previous.last_event.is_none()
                && row.last_event.is_none()
                && previous.service_connected == row.service_connected
                && previous.connected_devices == row.connected_devices
                && previous.frame.as_ref().zip(row.frame.as_ref()).is_some_and(|(a, b)| {
                    (a.hand_count, a.left.active, a.right.active) == (b.hand_count, b.left.active, b.right.active)
                })
        });
        if replace {
            *result.last_mut().unwrap() = row;
        } else {
            result.push(row);
        }
    }
    Ok(result)
}
fn validate(poll: &UltraleapRuntimePoll) -> Result<(), String> {
    if poll.connected_devices > 64 || poll.last_event.as_ref().is_some_and(|text| text.capacity() > 1024) {
        return Err("Ultraleap publication exceeds 64 devices / 1024 message bytes".into());
    }
    if let Some(frame) = &poll.frame {
        if frame.hand_count > 2 {
            return Err("Ultraleap frame exceeds two hands".into());
        }
        for hand in [&frame.left, &frame.right] {
            if ![hand.grab_strength, hand.pinch_strength, hand.pinch_distance]
                .into_iter()
                .all(f64::is_finite)
            {
                return Err("Ultraleap hand values are not finite".into());
            }
            for v in [
                hand.palm_position,
                hand.palm_stabilized_position,
                hand.palm_velocity,
                hand.palm_direction,
                hand.palm_normal,
            ] {
                if ![v.x, v.y, v.z].into_iter().all(f64::is_finite) {
                    return Err("Ultraleap vectors are not finite".into());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
