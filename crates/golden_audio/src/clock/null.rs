use crate::{AudioError, FrameCount, SampleRate};

#[derive(Clone, Debug)]
pub struct NullClockDriver {
    sample_rate: SampleRate,
    block_frames: FrameCount,
    maximum_catch_up_blocks: u32,
    next_deadline_nanos: Option<u128>,
}

impl NullClockDriver {
    pub fn new(
        sample_rate: SampleRate,
        block_frames: FrameCount,
        maximum_catch_up_blocks: u32,
    ) -> Result<Self, AudioError> {
        if maximum_catch_up_blocks == 0 {
            return Err(AudioError::invalid_configuration(
                "null clock catch-up limit must be greater than zero",
            ));
        }
        Ok(Self {
            sample_rate,
            block_frames,
            maximum_catch_up_blocks,
            next_deadline_nanos: None,
        })
    }

    #[must_use]
    pub fn poll(&mut self, now_nanos: u128) -> u32 {
        let block_nanos = self.block_nanos();
        let next = self
            .next_deadline_nanos
            .get_or_insert_with(|| now_nanos.saturating_add(block_nanos));
        if now_nanos < *next {
            return 0;
        }
        let elapsed_blocks = now_nanos.saturating_sub(*next) / block_nanos + 1;
        let due = elapsed_blocks.min(u128::from(self.maximum_catch_up_blocks)) as u32;
        *next = next.saturating_add(block_nanos.saturating_mul(u128::from(due)));
        due
    }

    pub fn resynchronize(&mut self, now_nanos: u128) {
        self.next_deadline_nanos = Some(now_nanos.saturating_add(self.block_nanos()));
    }

    #[must_use]
    pub const fn block_frames(&self) -> FrameCount {
        self.block_frames
    }

    fn block_nanos(&self) -> u128 {
        u128::from(self.block_frames.get()) * 1_000_000_000 / u128::from(self.sample_rate.get())
    }
}
