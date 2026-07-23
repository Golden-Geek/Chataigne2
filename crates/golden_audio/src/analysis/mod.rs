mod observation;
mod rms;
mod settings;

#[cfg(feature = "analysis")]
mod pipeline;
#[cfg(feature = "analysis")]
mod pitch;
#[cfg(feature = "analysis")]
mod spectrum;

pub use observation::{
    AnalysisDiagnosticsObservation, AnalysisObservationSnapshot, AnalysisResult, AnalysisTapObservation,
    ChannelObservation, PitchObservation, SpectrumBandObservation, SpectrumObservation,
};
pub use rms::{MeterAccumulator, linear_to_dbfs};
pub use settings::{
    AnalysisProcessorConfiguration, PitchAnalysisConfiguration, SpectrumAnalysisConfiguration, SpectrumBandSpacing,
    SpectrumOverlap, SpectrumWindow,
};

#[cfg(feature = "analysis")]
pub use pipeline::{
    AnalysisController, AnalysisObservationReader, AnalysisRenderer, AnalysisRendererRetirement, analysis_pipeline,
};
#[cfg(feature = "analysis")]
pub use pitch::PitchAnalyzer;
#[cfg(feature = "analysis")]
pub use spectrum::{SpectrumAnalyzer, SpectrumBandGeometry, spectrum_band_geometry};

#[cfg(test)]
mod tests;
