//! Software publication contents belong to their allocation across compatible plan rebuilds.
use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext;

use crate::TextureId;
use crate::image::ImageStream;
use crate::runtime::CompletionToken;

use super::super::texture::Texture;
use super::super::timing::{DiagnosticEvent, ImageUploadTrace, Recorder, SourceUseTrace};

pub(super) fn update(
    context: &ID3D11DeviceContext,
    target: &Texture,
    texture: TextureId,
    stream: &ImageStream,
    token: CompletionToken,
    recorder: Option<&Recorder>,
) {
    if target.image_sequence.get().is_none() {
        // A new allocation has no defined pixels until the first successful publication.
        unsafe { context.ClearRenderTargetView(&target.rtv, &[0.0; 4]) };
        target.last_write.set(Some(token));
        target.image_sequence.set(Some(0));
    }
    if let Some(sample) = stream.sample()
        && target.image_sequence.get() != Some(sample.sequence)
    {
        if let Some(image) = sample.image {
            let begin = recorder.and_then(Recorder::stamp);
            let row_pitch = image.descriptor().extent.width() * image.descriptor().format.bytes_per_pixel() as u32;
            // SAFETY: ImageData validates tight storage. The API consumes these CPU bytes
            // before returning; immediate-context ordering protects earlier GPU readers.
            unsafe {
                context.UpdateSubresource(&target.texture, 0, None, image.pixels().as_ptr().cast(), row_pitch, 0);
            }
            let cpu = recorder.and_then(|recorder| recorder.interval(begin));
            target.image_visible.set(sample.sequence);
            target
                .timed_frame
                .set(sample.frame.map(|frame| super::super::TimedFrameTrace {
                    pts_nanos: frame.window.pts().as_nanos(),
                    start_nanos: frame.window.start().as_nanos(),
                    end_nanos: frame.window.end().map(|end| end.as_nanos()),
                    publication_qpc: recorder.and_then(|recorder| recorder.publication_lower_bound(frame.published_at)),
                }));
            if let Some(recorder) = recorder {
                recorder.push(DiagnosticEvent::ImageUpload(ImageUploadTrace {
                    token,
                    texture,
                    stream: stream.id(),
                    sequence: sample.sequence,
                    row_pitch,
                    rows: image.descriptor().extent.height(),
                    bytes: image.pixels().len() as u64,
                    cpu,
                }));
            }
        } else {
            unsafe { context.ClearRenderTargetView(&target.rtv, &[0.0; 4]) };
            target.image_visible.set(0);
            target.timed_frame.set(None);
        }
        target.image_sequence.set(Some(sample.sequence));
        target.last_write.set(Some(token));
    }
    if let Some(recorder) = recorder {
        recorder.push(DiagnosticEvent::SourceUse(SourceUseTrace {
            token,
            texture,
            sequence: target.image_visible.get(),
            stream: Some(stream.id()),
            publication: None,
            software_upload: None,
            frame: target.timed_frame.get(),
            requested_sequence: None,
            requested_position_nanos: None,
        }));
    }
}
