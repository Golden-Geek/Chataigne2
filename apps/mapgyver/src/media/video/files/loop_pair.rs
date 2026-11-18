//! One companion decoder and bounded image preparation per demanded file channel.
use super::*;
use crate::media::video::TransportSample;
use golden_time::TimeEpoch;
use golden_video::{decode::DecodeStorage, scheduling::LoopBlend};

pub(super) struct LoopPair {
    incoming: Reader,
    lease: Arc<Lease>,
    phase: Option<(TimeEpoch, i64)>,
    image: Option<(Duration, Arc<golden_video::image::ImageData>)>,
}

impl LoopPair {
    pub(super) fn open(
        run: &Run,
        worker: &WorkerContext<(), ()>,
        reader: &Reader,
        primary_lease: &Lease,
    ) -> Result<Self, Failure> {
        let cancel = worker.cancellation().clone();
        let mut decoder = reader.open_companion(
            &run.file,
            DecodeLimits {
                max_pixels: 16384 * 16384,
                max_cpu_frame_bytes: MAX_BYTES,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 256,
                software_threads: golden_video::decode::SoftwareDecodeThreads::One,
            },
            move || cancel.is_cancelled(),
        )?;
        let lease = if reader.storage() == DecodeStorage::Software {
            run.budget.admit_software(&mut decoder)?
        } else {
            let lease = run
                .budget
                .reserve_bytes(primary_lease.bytes)
                .map_err(|error| Failure(VideoSourceError::new(VideoSourceErrorKind::Capacity, error)))?;
            if reader.storage() == DecodeStorage::Hap {
                decoder.tighten_cpu_frame_limit(lease.bytes - 16 * 1024 * 1024)?;
            }
            lease
        };
        let incoming = Reader::new(decoder)?;
        if incoming.duration != reader.duration {
            return Err("Video file changed while the loop companion opened".into());
        }
        Ok(Self {
            incoming,
            lease: Arc::new(lease),
            phase: None,
            image: None,
        })
    }

    /// The warmed incoming decoder takes over after its already-consumed section.
    pub(super) fn orient(&mut self, sample: TransportSample, primary: &mut Reader, lease: &mut Arc<Lease>) {
        if self.phase.is_some_and(|(epoch, cycle)| {
            sample.epoch == epoch && cycle.checked_add(if sample.reverse { -1 } else { 1 }) == Some(sample.cycle)
        }) {
            std::mem::swap(primary, &mut self.incoming);
            std::mem::swap(lease, &mut self.lease);
            self.image = None;
            primary.invalidate_publication();
            self.incoming.invalidate_publication();
        }
        self.phase = Some((sample.epoch, sample.cycle));
    }

    /// Warm one first/last sample before the overlap. Seeking and decoding remain cancellable
    /// bounded reader turns, and the incoming role never owns a public rendering stream.
    pub(super) fn prepare(&mut self, sample: TransportSample) -> Result<(bool, Option<LoopBlend>), Failure> {
        let blend = sample.loop_blend();
        let position = blend.map_or(
            if sample.reverse {
                self.incoming.duration
            } else {
                Duration::ZERO
            },
            |value| value.secondary_position,
        );
        self.incoming.advance(position)?;
        Ok((!self.incoming.catching_up, blend))
    }

    pub(super) fn reader(&self) -> &Reader {
        &self.incoming
    }

    pub(super) fn software_image(&mut self) -> Result<Arc<golden_video::image::ImageData>, Failure> {
        let position = self
            .incoming
            .neighbours()?
            .ok_or("Video loop head timestamp unavailable")?
            .position;
        if self.image.as_ref().is_none_or(|(stamp, _)| *stamp != position) {
            let image = self.incoming.software_image()?.retain(self.lease.clone());
            self.image = Some((position, Arc::new(image)));
        }
        Ok(self.image.as_ref().expect("cached loop image").1.clone())
    }

    pub(super) fn blend_software(
        &mut self,
        run: &Run,
        primary: golden_video::image::ImageData,
        blend: LoopBlend,
    ) -> Result<golden_video::image::ImageData, Failure> {
        use golden_video::image::{ImageError, ImageStorageBudget, crossfade_srgba8};
        let bytes = primary.pixels().len();
        let capacity = |error: String| Failure(VideoSourceError::new(VideoSourceErrorKind::Capacity, error));
        let lease = run.budget.reserve_bytes(bytes as u64).map_err(capacity)?;
        let budget = ImageStorageBudget::new(bytes as u64).map_err(|error| capacity(error.to_string()))?;
        let incoming = self.software_image()?;
        let weight = golden_video::Opacity::new(blend.secondary_weight).map_err(|error| error.to_string())?;
        let result = crossfade_srgba8(&primary, &incoming, weight, &budget).map_err(|error| match error {
            ImageError::Capacity => capacity(error.to_string()),
            _ => Failure::from(error.to_string()),
        })?;
        Ok(result.retain(Arc::new(lease)))
    }
}
