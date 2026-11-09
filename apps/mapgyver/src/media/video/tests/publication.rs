use super::*;
use crate::media::protocol::{AssetLocator, ChannelId};
use crate::media::video::protocol::VideoTransportSettings;
use golden_core::node::NodeUuid;
use golden_video::{
    decode::{DecodeLimits, Decoder, SoftwareImageConverter},
    image::{ImageData, ImageStream},
};
use std::time::Instant;

fn open(name: &str) -> Decoder {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/golden_video/src/decode/tests/fixtures");
    Decoder::open(
        &root.join(name),
        DecodeLimits {
            max_pixels: 4096 * 2160,
            max_packet_bytes: 1024 * 1024,
            max_packets_per_frame: 256,
            software_threads: golden_video::decode::SoftwareDecodeThreads::One,
        },
    )
    .unwrap()
}

fn decoded_at(reader: &mut super::super::reader::Reader, position: Duration) -> ImageData {
    for _ in 0..64 {
        if let Some(image) = reader.poll(position).unwrap() {
            return image;
        }
    }
    panic!("bounded reader did not reach {position:?}");
}

#[test]
fn superseding_seeks_and_file_incarnations_reject_held_real_decodes_before_stream_mutation() {
    for name in [
        "h264.mp4",
        "h264-keyframes.mp4",
        "h264-color.mp4",
        "hevc.mp4",
        "hevc-main.mp4",
        "hevc-main10.mp4",
        "prores.mov",
        "prores-alpha.mov",
    ] {
        let mut decoder = open(name);
        let base = decoder.info().time_base;
        let mut converter = SoftwareImageConverter::default();
        let mut expected = None;
        while let Some(frame) = decoder.next_frame().unwrap() {
            let time = super::super::reader::duration(frame.timestamp(), base).unwrap();
            if time <= Duration::from_millis(125) {
                expected = Some(converter.convert(&frame).unwrap().pixels().to_vec());
            }
        }
        let started = Instant::now();
        let mut reader = super::super::reader::Reader::new(open(name)).unwrap();
        let held = decoded_at(&mut reader, Duration::ZERO);
        let cold = started.elapsed();
        let channel = ChannelId(NodeUuid(uuid::Uuid::new_v4()));
        let file = AssetLocator::ProjectRelative {
            path: name.to_owned().try_into().unwrap(),
        };
        let control = VideoControl::default();
        let binding = uuid::Uuid::new_v4();
        let owner = uuid::Uuid::new_v4();
        control.bind_file(channel, binding, Some(file.clone())).unwrap();
        control.claim_channel(channel, owner, file.clone()).unwrap();
        control.channel_duration(channel, owner, reader.duration).unwrap();
        let stream = ImageStream::new(held.descriptor());
        let mut superseded = vec![control.sample().unwrap()];
        for index in 0..1024 {
            control
                .configure(
                    VideoTransportSettings {
                        position_ms: if index % 2 == 0 { 0.0 } else { 180.0 },
                        ..Default::default()
                    },
                    true,
                )
                .unwrap();
            if index % 64 == 0 {
                superseded.push(control.sample().unwrap());
            }
        }
        control
            .configure(
                VideoTransportSettings {
                    position_ms: 125.0,
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        for stale in superseded {
            let image = ImageData::new(held.descriptor(), held.pixels().to_vec()).unwrap();
            assert!(!control.publish_frame(channel, owner, stale, &stream, image).unwrap());
            assert_eq!(stream.sample().unwrap().sequence, 0);
        }
        let latest = control.sample().unwrap();
        assert_eq!(latest.position, Duration::from_millis(125));
        let started = Instant::now();
        let image = decoded_at(&mut reader, latest.position);
        let warm = started.elapsed();
        assert_eq!(image.pixels(), expected.unwrap(), "{name}: latest exact pixels");
        assert!(control.publish_frame(channel, owner, latest, &stream, image).unwrap());
        assert_eq!(stream.sample().unwrap().sequence, 1);

        let replacement = uuid::Uuid::new_v4();
        control.claim_channel(channel, replacement, file.clone()).unwrap();
        let sample = control.sample().unwrap();
        assert!(!control.publish_frame(channel, owner, sample, &stream, held).unwrap());
        assert_eq!(stream.sample().unwrap().sequence, 1);
        control.bind_file(channel, binding, None).unwrap();
        let pixels = stream.sample().unwrap().image.unwrap().pixels().to_vec();
        let image = ImageData::new(stream.descriptor(), pixels).unwrap();
        assert!(
            !control
                .publish_frame(channel, replacement, sample, &stream, image)
                .unwrap()
        );
        assert_eq!(stream.sample().unwrap().sequence, 1);
        eprintln!("{name}:1025 seeks coalesced,17 held epochs/old owner/relink rejected; cold={cold:?},warm={warm:?}");
    }
}
