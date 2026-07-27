use std::path::Path;

use ts_rs::{Config, TS};

use crate::{
    AnalysisObservationSnapshot, AnalysisTapConfiguration, AudioDeviceInspectorState, AudioDeviceSelection,
    AudioFileFormat, PlaybackObservation, RenderRuntimeObservation, supported_audio_extensions,
};

const INDEX: &str = "\
export type { AnalysisDiagnosticsObservation } from './AnalysisDiagnosticsObservation';\n\
export type { AnalysisObservationSnapshot } from './AnalysisObservationSnapshot';\n\
export type { AnalysisProcessorConfiguration } from './AnalysisProcessorConfiguration';\n\
export type { AnalysisResult } from './AnalysisResult';\n\
export type { AnalysisTapConfiguration } from './AnalysisTapConfiguration';\n\
export type { AnalysisTapId } from './AnalysisTapId';\n\
export type { AnalysisTapObservation } from './AnalysisTapObservation';\n\
export type { AudioBackendState } from './AudioBackendState';\n\
export type { AudioBackendStatus } from './AudioBackendStatus';\n\
export type { AudioBufferPolicy } from './AudioBufferPolicy';\n\
export type { AudioChannelId } from './AudioChannelId';\n\
export type { AudioDeviceDescriptor } from './AudioDeviceDescriptor';\n\
export type { AudioDeviceCatalogEntry } from './AudioDeviceCatalogEntry';\n\
export type { AudioDeviceFingerprint } from './AudioDeviceFingerprint';\n\
export type { AudioDeviceId } from './AudioDeviceId';\n\
export type { AudioDeviceInspectorState } from './AudioDeviceInspectorState';\n\
export type { AudioDeviceProfileKey } from './AudioDeviceProfileKey';\n\
export type { AudioDeviceReadiness } from './AudioDeviceReadiness';\n\
export type { AudioDeviceSelection } from './AudioDeviceSelection';\n\
export type { AudioDeviceTargetId } from './AudioDeviceTargetId';\n\
export type { AudioDirection } from './AudioDirection';\n\
export type { AudioErrorCategory } from './AudioErrorCategory';\n\
export type { AudioFileFormat } from './AudioFileFormat';\n\
export type { AudioInspectorError } from './AudioInspectorError';\n\
export type { AudioPermissionState } from './AudioPermissionState';\n\
export type { AudioRecoveryPolicy } from './AudioRecoveryPolicy';\n\
export type { AudioSampleFormat } from './AudioSampleFormat';\n\
export type { AudioStreamStatus } from './AudioStreamStatus';\n\
export type { BackendId } from './BackendId';\n\
export type { ChannelObservation } from './ChannelObservation';\n\
export type { ConfigGeneration } from './ConfigGeneration';\n\
export type { NegotiatedStreamFormat } from './NegotiatedStreamFormat';\n\
export type { PhysicalChannelDescriptor } from './PhysicalChannelDescriptor';\n\
export type { PhysicalChannelKey } from './PhysicalChannelKey';\n\
export type { PitchAnalysisConfiguration } from './PitchAnalysisConfiguration';\n\
export type { PitchObservation } from './PitchObservation';\n\
export type { PlaybackObservation } from './PlaybackObservation';\n\
export type { RenderRuntimeObservation } from './RenderRuntimeObservation';\n\
export type { SpectrumAnalysisConfiguration } from './SpectrumAnalysisConfiguration';\n\
export type { SpectrumBandObservation } from './SpectrumBandObservation';\n\
export type { SpectrumBandSpacing } from './SpectrumBandSpacing';\n\
export type { SpectrumObservation } from './SpectrumObservation';\n\
export type { SpectrumOverlap } from './SpectrumOverlap';\n\
export type { SpectrumWindow } from './SpectrumWindow';\n\
export type { SupportedBufferFrames } from './SupportedBufferFrames';\n\
export type { SupportedStreamConfiguration } from './SupportedStreamConfiguration';\n\
export { supportedAudioExtensions } from './supportedAudioExtensions';\n\
export type { SupportedAudioExtension } from './supportedAudioExtensions';\n";

pub fn export_device_contract(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir)?;
    let config = Config::new().with_out_dir(output_dir.to_path_buf());
    AudioDeviceInspectorState::export_all(&config)?;
    AudioDeviceSelection::export_all(&config)?;
    AudioFileFormat::export_all(&config)?;
    AnalysisTapConfiguration::export_all(&config)?;
    AnalysisObservationSnapshot::export_all(&config)?;
    PlaybackObservation::export_all(&config)?;
    RenderRuntimeObservation::export_all(&config)?;
    let extensions = supported_audio_extensions()
        .iter()
        .map(|extension| format!("'{extension}'"))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        output_dir.join("supportedAudioExtensions.ts"),
        format!(
            "export const supportedAudioExtensions = [{extensions}] as const;\n\
             export type SupportedAudioExtension = (typeof supportedAudioExtensions)[number];\n"
        ),
    )?;
    std::fs::write(output_dir.join("index.ts"), INDEX)?;
    Ok(())
}
