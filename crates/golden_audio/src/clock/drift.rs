use crate::AudioError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriftControllerConfig {
    pub target_fill_frames: usize,
    pub proportional_ppm_per_frame: f64,
    pub integral_ppm_per_frame: f64,
    pub maximum_correction_ppm: f64,
    pub integral_limit_frames: f64,
}

impl DriftControllerConfig {
    pub fn validate(self) -> Result<(), AudioError> {
        if self.target_fill_frames == 0 {
            return Err(AudioError::invalid_configuration(
                "drift controller target fill must be greater than zero",
            ));
        }
        for (name, value) in [
            ("proportional_ppm_per_frame", self.proportional_ppm_per_frame),
            ("integral_ppm_per_frame", self.integral_ppm_per_frame),
            ("maximum_correction_ppm", self.maximum_correction_ppm),
            ("integral_limit_frames", self.integral_limit_frames),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(AudioError::invalid_configuration(format!(
                    "drift controller {name} must be finite and positive"
                )));
            }
        }
        Ok(())
    }
}

impl Default for DriftControllerConfig {
    fn default() -> Self {
        Self {
            target_fill_frames: 2_048,
            proportional_ppm_per_frame: 0.75,
            integral_ppm_per_frame: 0.0025,
            maximum_correction_ppm: 2_000.0,
            integral_limit_frames: 400_000.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DriftController {
    config: DriftControllerConfig,
    integral_error_frames: f64,
    correction_ppm: f64,
}

impl DriftController {
    pub fn new(config: DriftControllerConfig) -> Result<Self, AudioError> {
        config.validate()?;
        Ok(Self {
            config,
            integral_error_frames: 0.0,
            correction_ppm: 0.0,
        })
    }

    pub fn observe_fill(&mut self, fill_frames: usize) -> f64 {
        let error = fill_frames as f64 - self.config.target_fill_frames as f64;
        self.integral_error_frames = (self.integral_error_frames + error)
            .clamp(-self.config.integral_limit_frames, self.config.integral_limit_frames);
        self.correction_ppm = (self.config.proportional_ppm_per_frame * error
            + self.config.integral_ppm_per_frame * self.integral_error_frames)
            .clamp(-self.config.maximum_correction_ppm, self.config.maximum_correction_ppm);
        self.correction_ppm
    }

    pub fn reset(&mut self) {
        self.integral_error_frames = 0.0;
        self.correction_ppm = 0.0;
    }

    #[must_use]
    pub const fn target_fill_frames(&self) -> usize {
        self.config.target_fill_frames
    }

    #[must_use]
    pub fn correction_ppm(&self) -> f64 {
        self.correction_ppm
    }
}
