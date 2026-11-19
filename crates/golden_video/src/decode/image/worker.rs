//! One software frame projection owned by the caller's file-worker lifetime.
use super::{
    ConversionError, DecodedFrame, ImageData, ImageStorageBudget, SoftwareImageConverter, ffmpeg, software_planes,
};
use crate::decode::SoftwareFrameLayout;
use golden_io::WorkerTask;
use std::sync::mpsc::{self, Receiver, Sender};

type Converted = Result<(ImageData, Option<SoftwareFrameLayout>), ConversionError>;

struct ConvertJob {
    frame: ffmpeg::frame::Video,
    completed: Sender<Converted>,
}

/// Software scaler with one in-flight conversion and an allocation-time payload ceiling.
/// Create and drop on the owning IO worker. Dropping retires the scaler and joins its thread.
pub struct SoftwareConversionWorker {
    task: Option<WorkerTask<ConvertJob>>,
    budget: ImageStorageBudget,
    scratch: Option<SoftwareFrameLayout>,
}

/// An outstanding projection. Consuming it delivers the image, and dropping it waits
/// for that one projection to retire. Both operations belong on the owning IO worker.
pub struct PendingSoftwareConversion<'a> {
    worker: &'a mut SoftwareConversionWorker,
    completed: Option<Receiver<Converted>>,
}

impl SoftwareConversionWorker {
    /// Use the same storage ceiling as the inline scaler.
    pub fn new(max_bytes: u64) -> Result<Self, ConversionError> {
        let budget = ImageStorageBudget::new(max_bytes).map_err(|_| ConversionError::Capacity)?;
        let worker_budget = budget.clone();
        let task = WorkerTask::spawn_with_capacity("golden-video-color", 1, move |jobs| {
            let mut converter = SoftwareImageConverter::with_budget(worker_budget);
            for job in jobs {
                let ConvertJob { frame, completed } = job;
                let converted = converter
                    .convert_planes(&frame)
                    .map(|image| (image, converter.scratch_layout()));
                drop(frame);
                let _ = completed.send(converted);
            }
        })
        .map_err(|error| ConversionError::Failed(error.to_string()))?;
        Ok(Self {
            task: Some(task),
            budget,
            scratch: None,
        })
    }

    /// Start conversion with a reference to the already admitted software buffers.
    /// The returned ticket prevents submitting another image while this one runs.
    pub fn begin(&mut self, frame: &DecodedFrame) -> Result<PendingSoftwareConversion<'_>, ConversionError> {
        let source = software_planes(frame)?;
        let mut retained = ffmpeg::frame::Video::empty();
        // SAFETY: the destination is an empty owned AVFrame. FFmpeg retains the
        // source's refcounted software buffers before the owning decoder continues.
        let result = unsafe { ffmpeg::ffi::av_frame_ref(retained.as_mut_ptr(), source.as_ptr()) };
        if result < 0 {
            return Err(ConversionError::Failed(ffmpeg::Error::from(result).to_string()));
        }
        let (completed, receiver) = mpsc::channel();
        self.task
            .as_ref()
            .expect("live conversion worker")
            .send(ConvertJob {
                frame: retained,
                completed,
            })
            .map_err(|_| ConversionError::Failed("Video conversion worker could not accept the frame".into()))?;
        Ok(PendingSoftwareConversion {
            worker: self,
            completed: Some(receiver),
        })
    }

    /// Read-only observer over scratch and published images.
    pub fn storage_budget(&self) -> ImageStorageBudget {
        self.budget.clone()
    }

    /// Last observed conversion scratch layout.
    pub fn scratch_layout(&self) -> Option<SoftwareFrameLayout> {
        self.scratch
    }
}

impl PendingSoftwareConversion<'_> {
    /// Wait for the immutable image, preserving normal conversion errors.
    pub fn finish(mut self) -> Result<ImageData, ConversionError> {
        let (image, scratch) = self
            .completed
            .take()
            .expect("live conversion ticket")
            .recv()
            .map_err(|_| ConversionError::Failed("Video conversion worker exited".into()))??;
        self.worker.scratch = scratch;
        Ok(image)
    }
}

impl Drop for PendingSoftwareConversion<'_> {
    fn drop(&mut self) {
        if let Some(receiver) = self.completed.take() {
            let _ = receiver.recv();
        }
    }
}

impl Drop for SoftwareConversionWorker {
    fn drop(&mut self) {
        if let Some(task) = self.task.take()
            && let Some(thread) = task.into_join_handle()
        {
            let _ = thread.join();
        }
    }
}
