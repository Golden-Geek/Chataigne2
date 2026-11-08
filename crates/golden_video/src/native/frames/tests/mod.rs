use super::*;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};
#[cfg(feature = "ffmpeg")]
mod synthetic;
#[cfg(feature = "ffmpeg")]
pub(in crate::native) use synthetic::{completed, prepared, submit};
fn descriptor() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(64, 48).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    }
}
#[test]
fn gpu_stream_withdrawal_and_contention_are_distinct() {
    let stream = GpuFrameStream::new(descriptor(), AdapterId { low: 1, high: 0 });
    assert_eq!(stream.sample().unwrap().sequence, 0);
    stream.publish(None).unwrap();
    let sample = stream.sample().unwrap();
    assert_eq!(sample.sequence, 1);
    assert!(sample.image.is_none());
    let _guard = stream.state.lock().unwrap();
    assert!(stream.sample().is_none());
}
#[test]
fn gpu_budget_includes_retained_lease_observers() {
    let budget = GpuFrameBudget::new(64);
    let lease = Arc::new(budget.0.claim(64).unwrap());
    let held = lease.clone();
    drop(lease);
    assert_eq!(budget.retained_bytes(), 64);
    assert!(budget.0.claim(1).is_none());
    drop(held);
    assert_eq!(budget.retained_bytes(), 0);
}
