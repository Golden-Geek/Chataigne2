use crate::image::{ImageData, ImageStream};
use crate::native::backend::input_observation::{Observation, capture};
use crate::plan::{ExternalSource, ExternalTexture};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, TextureId};
use std::{sync::Arc, time::Instant};

fn descriptor() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(2, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Straight,
    }
}
fn source(stream: &ImageStream) -> [ExternalTexture; 1] {
    [ExternalTexture {
        id: TextureId::new(1).unwrap(),
        descriptor: stream.descriptor(),
        source: Some(ExternalSource::Frames(stream.clone())),
    }]
}

#[test]
fn cpu_input_observation_retains_immutable_pixels_until_the_frame_consumes_them() {
    let stream = ImageStream::new(descriptor());
    stream
        .publish(Some(ImageData::new(descriptor(), vec![7; 8]).unwrap()))
        .unwrap();
    let weak = Arc::downgrade(&stream.sample().unwrap().image.unwrap());
    let external = source(&stream);
    let mut observed = capture(&external, Instant::now());
    stream
        .publish(Some(ImageData::new(descriptor(), vec![9; 8]).unwrap()))
        .unwrap();
    assert!(weak.upgrade().is_some());
    let Observation::Image(observed) = observed.next().unwrap() else {
        panic!("expected software image observation")
    };
    assert_eq!(observed.stream.id(), stream.id());
    let sample = observed.sample.unwrap();
    assert_eq!(sample.sequence, 1);
    let image = sample.image.unwrap();
    assert_eq!(image.pixels(), &[7; 8]);
    assert_eq!(stream.sample().unwrap().image.unwrap().pixels(), &[9; 8]);
    drop(image);
    assert!(
        weak.upgrade().is_none(),
        "the snapshot must not retain an extra frame after consumption"
    );
}

#[test]
fn captured_withdrawal_cannot_be_replaced_by_a_later_publication_in_the_same_frame() {
    let stream = ImageStream::new(descriptor());
    stream.publish(None).unwrap();
    let external = source(&stream);
    let mut observed = capture(&external, Instant::now());
    stream
        .publish(Some(ImageData::new(descriptor(), vec![9; 8]).unwrap()))
        .unwrap();
    let Observation::Image(observed) = observed.next().unwrap() else {
        panic!("expected software withdrawal")
    };
    let sample = observed.sample.unwrap();
    assert_eq!(sample.sequence, 1);
    assert!(sample.image.is_none());
    assert_eq!(stream.sample().unwrap().sequence, 2);
}
