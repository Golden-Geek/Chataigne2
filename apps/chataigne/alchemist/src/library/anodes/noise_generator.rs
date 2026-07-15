use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{
    cellular_noise_1d, config_string, delta_seconds, float_inputs, fractal_noise_1d, gradient_noise_1d, hash_noise,
    set_state_values, state_values,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NoiseAlgorithm {
    Random,
    Perlin,
    Simplex,
    Brownian,
    Cellular,
    Fractal,
}

impl NoiseAlgorithm {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "algorithm", "random").as_str() {
            "perlin" => Self::Perlin,
            "simplex" => Self::Simplex,
            "brownian" => Self::Brownian,
            "cellular" => Self::Cellular,
            "fractal" => Self::Fractal,
            _ => Self::Random,
        }
    }
}

#[derive(Debug)]
pub(super) struct NoiseGeneratorEval {
    pub(super) algorithm: NoiseAlgorithm,
    pub(super) scale: f64,
    pub(super) seed: i64,
    pub(super) octaves: usize,
    pub(super) persistence: f64,
    pub(super) lacunarity: f64,
    pub(super) jitter: f64,
}

impl CompiledNodeEvaluator for NoiseGeneratorEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [position] = float_inputs::<1>(evaluation.inputs)?;
        let dt = delta_seconds(evaluation.ctx.delta_time);
        let state = evaluation.state.first_mut();
        let mut values = state_values(state.as_deref(), 2);
        values[0] += dt;
        let x = (position + values[0]) * self.scale;
        let output = match self.algorithm {
            NoiseAlgorithm::Random => hash_noise(self.seed, evaluation.ctx.logical_tick as i64),
            NoiseAlgorithm::Perlin => gradient_noise_1d(self.seed, x),
            NoiseAlgorithm::Simplex => gradient_noise_1d(self.seed ^ 0x5EED, x * 1.309 + 19.19),
            NoiseAlgorithm::Brownian => {
                let step = fractal_noise_1d(self.seed, x, self.octaves, self.persistence, self.lacunarity);
                values[1] = (values[1] + step * dt.sqrt()).clamp(-1.0, 1.0);
                values[1]
            }
            NoiseAlgorithm::Cellular => cellular_noise_1d(self.seed, x, self.jitter),
            NoiseAlgorithm::Fractal => fractal_noise_1d(self.seed, x, self.octaves, self.persistence, self.lacunarity),
        };
        set_state_values(state, values);
        Ok(vec![RuntimeValue::Float(output.clamp(-1.0, 1.0))])
    }
}
