use super::*;
use crate::image::ImageData;
use crate::native::backend::input_observation::{Observation, capture};
use crate::native::{GpuFrameClock, GpuFramePublication};
use crate::scheduling::{FramePosition, FrameWindow};
use golden_time::TimeEpoch;
use std::sync::Arc;

struct Clock {
    origin: Instant,
    epoch: TimeEpoch,
}
impl GpuFrameClock for Clock {
    fn position_at(&self, instant: Instant) -> Option<FramePosition> {
        Some(FramePosition {
            epoch: self.epoch,
            position: ShowPosition::from_nanos(
                instant
                    .checked_duration_since(self.origin)?
                    .as_nanos()
                    .try_into()
                    .ok()?,
            ),
            reverse: false,
        })
    }
}

#[test]
#[ignore = "explicit Intel captured-instant eviction, immutable pixels and bounded source retention"]
fn hardware_intel_input_observation_survives_window_eviction_before_upload() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| adapter.vendor_id == 0x8086 && !adapter.software)
        .unwrap();
    let desc = descriptor(2, ColorEncoding::Linear, AlphaMode::Straight);
    let budget = GpuFrameBudget::new(24);
    let mut owner = GpuFramePublisher::new_software(desc, adapter.id, &budget, None).unwrap();
    let mut authority =
        TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let epoch = authority.sample().unwrap().epoch();
    let origin = Instant::now();
    owner.set_presentation_clock(Arc::new(Clock { origin, epoch })).unwrap();
    let stream = owner.stream();
    drop(stream.begin_epoch(epoch).unwrap());
    let at = |nanos| FramePosition {
        epoch,
        position: ShowPosition::from_nanos(nanos),
        reverse: false,
    };
    let window = |start, end| {
        FrameWindow::new(
            epoch,
            ShowPosition::from_nanos(start),
            Some(ShowPosition::from_nanos(end)),
            ShowPosition::from_nanos(start),
        )
        .unwrap()
    };
    let pixels = |pixel| Arc::new(ImageData::new(desc, [pixel, pixel].concat()).unwrap());
    assert!(owner.submit_software(pixels([255, 0, 0, 255])).unwrap());
    let red = completed(&mut owner);
    assert!(owner.submit_software(pixels([0, 255, 0, 255])).unwrap());
    let green = completed(&mut owner);
    assert!(matches!(
        stream.replace_scheduled(red, window(0, 10), at(0)).unwrap(),
        GpuFramePublication::Published(_)
    ));
    let external = [ExternalTexture {
        id: TextureId::new(1).unwrap(),
        descriptor: desc,
        source: Some(ExternalSource::GpuFrames(stream.clone())),
    }];
    let instant = origin + Duration::from_nanos(5);
    let mut observed = capture(&external, instant);
    // Simulate another input's work letting the publisher retire the due old window.
    assert!(matches!(
        stream.replace_scheduled(green, window(10, 20), at(10)).unwrap(),
        GpuFramePublication::Published(_)
    ));
    assert!(
        stream.sample_at(instant).is_none(),
        "the old due window really was evicted"
    );
    let Observation::Gpu {
        sample: Some(mut sample),
        ..
    } = observed.next().unwrap()
    else {
        panic!("frame observation was deferred until after the due source window expired")
    };
    assert_eq!(sample.frame.unwrap().pts_nanos, 0);
    let fixture = prepared_image(desc, &[0; 8]);
    let mut renderer = Renderer::new(adapter.id, &fixture, 2);
    let held = GpuFrameStream::new(desc, adapter.id);
    held.publish(sample.image.take()).unwrap();
    install(&mut renderer, held.clone());
    assert_eq!(renderer.pixel_frame(), vec![[255, 0, 0, 255]; 2]);
    let fresh = stream.sample_at(origin + Duration::from_nanos(15)).unwrap();
    assert_eq!(fresh.frame.unwrap().pts_nanos, 10);
    held.publish(fresh.image).unwrap();
    assert_eq!(renderer.pixel_frame(), vec![[0, 255, 0, 255]; 2]);
    assert_eq!(
        budget.retained_bytes(),
        24,
        "captured views share the original admitted storage"
    );
    drop(renderer);
    drop(held);
    drop(observed);
    drop(external);
    drop(stream);
    drop(owner);
    assert_eq!(budget.retained_bytes(), 0);
}
