use super::*;
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

fn image(width: u32, byte: u8) -> ImageData {
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(width, 1).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        alpha_mode: AlphaMode::Opaque,
        color_encoding: ColorEncoding::Srgb,
    };
    ImageData::new(descriptor, vec![byte; 4 * width as usize]).unwrap()
}

#[test]
fn frames_reuse_one_stream_and_withdraw_on_format_change_or_disconnect() {
    let output = Arc::new(Mutex::new(Output::default()));
    let producer = Producer {
        sender: Arc::from("Camera (Stage)"),
        output: output.clone(),
        notify: None,
    };
    let stream = || output.lock().unwrap().stream.clone();
    assert!(producer.publish(image(1, 11)));
    let first = stream().unwrap();
    assert!(producer.publish(image(1, 22)));
    assert_eq!(stream().unwrap().id(), first.id());
    assert_eq!(first.sample().unwrap().image.unwrap().pixels(), [22; 4]);

    assert!(producer.publish(image(2, 33)));
    let second = stream().unwrap();
    assert_ne!(second.id(), first.id());
    assert!(first.sample().unwrap().image.is_none());
    assert_eq!(second.sample().unwrap().image.unwrap().pixels(), [33; 8]);
    producer.withdraw();
    assert!(stream().is_none());
    assert!(second.sample().unwrap().image.is_none());
}
