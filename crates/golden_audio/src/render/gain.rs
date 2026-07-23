#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GainSmoother {
    current: f32,
    target: f32,
    step: f32,
    remaining: u32,
}

impl GainSmoother {
    #[must_use]
    pub const fn settled(gain: f32) -> Self {
        Self {
            current: gain,
            target: gain,
            step: 0.0,
            remaining: 0,
        }
    }

    #[must_use]
    pub const fn current(self) -> f32 {
        self.current
    }

    #[must_use]
    pub const fn target(self) -> f32 {
        self.target
    }

    #[must_use]
    pub const fn remaining(self) -> u32 {
        self.remaining
    }

    pub fn set_target(&mut self, target: f32, frames: u32) {
        debug_assert!(target.is_finite());
        self.target = target;
        if frames == 0 {
            self.current = target;
            self.step = 0.0;
            self.remaining = 0;
            return;
        }
        self.step = (target - self.current) / frames as f32;
        self.remaining = frames;
    }

    #[must_use]
    pub fn next_gain(&mut self) -> f32 {
        if self.remaining > 0 {
            self.current += self.step;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.current = self.target;
                self.step = 0.0;
            }
        }
        self.current
    }
}
