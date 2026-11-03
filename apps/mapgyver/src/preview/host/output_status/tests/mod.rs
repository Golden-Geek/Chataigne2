use super::*;
use golden_video::{native::OutputState, runtime::CompletionToken};
use std::time::Duration;
#[test]
fn each_output_uses_its_own_counter_and_replacement_or_removal_discards_baselines() {
    let first = ScreenId(golden_core::model::NodeUuid(uuid::Uuid::new_v4()));
    let second = ScreenId(golden_core::model::NodeUuid(uuid::Uuid::new_v4()));
    let runtime = uuid::Uuid::new_v4();
    let generation = |value| {
        serde_json::from_value::<DeviceGeneration>(serde_json::json!({ "runtime": runtime, "generation": value }))
            .unwrap()
    };
    let device = generation(1);
    let extent = golden_video::Extent2D::new(640, 360).unwrap();
    let routes = [
        (first, OutputId::new(2).unwrap(), extent),
        (second, OutputId::new(3).unwrap(), extent),
    ];
    let make = |id| OutputSnapshot {
        id,
        last_accepted: Some(CompletionToken { device, serial: 100 }),
        accepted_frames: 0,
        state: OutputState::Ready,
        generation: 1,
        attempts: 1,
        hresult: None,
        allocation: None,
    };
    let mut outputs = [make(routes[0].1), make(routes[1].1)];
    let mut sampler = OutputSampler::default();
    let start = Instant::now();
    assert!(
        sampler
            .sample(start, &routes, &outputs)
            .iter()
            .all(|row| row.accepted_fps.is_none())
    );
    outputs[0].accepted_frames = 60;
    outputs[1].accepted_frames = 24;
    let next = sampler.sample(start + Duration::from_secs(1), &routes, &outputs);
    assert_eq!(next[0].accepted_fps, Some(60.0));
    assert_eq!(next[1].accepted_fps, Some(24.0));
    outputs[0].generation += 1;
    outputs[0].accepted_frames = 1;
    let next = sampler.sample(start + Duration::from_secs(2), &routes, &outputs);
    assert_eq!(next[0].accepted_fps, None);
    assert_eq!(next[1].accepted_fps, Some(0.0));
    outputs[0].last_accepted.as_mut().unwrap().device = generation(2);
    assert_eq!(
        sampler.sample(start + Duration::from_secs(3), &routes, &outputs)[0].accepted_fps,
        None
    );
    assert!(sampler.sample(start + Duration::from_secs(4), &[], &[]).is_empty());
    assert!(sampler.previous.is_empty());
    assert!(
        sampler
            .sample(start + Duration::from_secs(5), &routes, &outputs)
            .iter()
            .all(|row| row.accepted_fps.is_none())
    );
}
