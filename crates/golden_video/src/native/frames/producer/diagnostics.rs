//! Opt-in producer observations reuse the renderer's bounded queue and query machinery.
use super::{GpuFrameError, GpuFramePublisher};
use crate::native::{
    DiagnosticContext, DiagnosticEvent, SourceCpuTrace, SourceSubmissionSink,
    timing::{CpuScope, GpuTiming, Recorder},
};
use crate::runtime::CompletionToken;
use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext;

pub(super) struct ProducerDiagnostics {
    pub(super) gpu: GpuTiming,
    pub(super) completed: CompletionToken,
    recorder: Recorder,
    context: DiagnosticContext,
}

pub(super) struct ProducerCpuScope {
    scope: Option<CpuScope>,
    recorder: Recorder,
    context: DiagnosticContext,
    token: CompletionToken,
}

impl ProducerDiagnostics {
    pub(super) fn cpu_scope(&self, token: CompletionToken) -> ProducerCpuScope {
        ProducerCpuScope {
            scope: Some(CpuScope::new(&self.recorder)),
            recorder: self.recorder.clone(),
            context: self.context,
            token,
        }
    }

    pub(super) fn poll(&mut self, context: &ID3D11DeviceContext) {
        self.gpu.poll(context, self.completed);
    }

    pub(super) fn finish(&mut self, context: &ID3D11DeviceContext) {
        // Only actual fence observations authorize query collection. Device removal leaves
        // incomplete tokens explicit; query retirement never grants image-retirement authority.
        self.gpu.finish(context, self.completed);
        self.gpu.abandon();
    }
}

impl Drop for ProducerCpuScope {
    fn drop(&mut self) {
        let measurement = self.scope.take().unwrap().finish();
        self.recorder.push(DiagnosticEvent::SourceCpu(SourceCpuTrace {
            token: self.token,
            context: self.context,
            wall: measurement.wall,
            thread_cpu: measurement.thread_cpu,
            receive_api: None,
            bridge: None,
            completion: None,
        }));
    }
}

impl GpuFramePublisher {
    /// Enable bounded source CPU/GPU diagnostics before the first submission. Query failures
    /// remain diagnostic rows; they cannot fail frame admission or change playback fallback.
    /// Use the same sink for submission facts and its eventual terminal drain.
    pub fn enable_diagnostics(&mut self, sink: &SourceSubmissionSink) -> Result<(), GpuFrameError> {
        if self.serial != 0 || self.pending.is_some() || self.diagnostics.is_some() {
            return Err(GpuFrameError::Layout);
        }
        let context = DiagnosticContext::FrameProducer {
            stream: self.stream.id(),
        };
        self.diagnostics = Some(ProducerDiagnostics {
            gpu: GpuTiming::new(&self.device.device, sink.0.clone(), context),
            completed: CompletionToken {
                device: self.generation,
                serial: 0,
            },
            recorder: sink.0.clone(),
            context,
        });
        self.record_software_upload_timing();
        Ok(())
    }
}
