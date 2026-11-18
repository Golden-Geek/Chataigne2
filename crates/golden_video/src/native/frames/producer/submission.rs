//! Submission facts are read from the actual pending input, never inferred from sampling.
use super::{GpuFramePublisher, Input};
use crate::native::{SourceSubmissionPayload, SourceSubmissionSink, SourceSubmissionTrace};
use crate::scheduling::FrameWindow;

impl GpuFramePublisher {
    /// Trace the current successful submission at most once, before polling completion.
    /// Returns false if there is no pending submission or it was already traced. A full
    /// diagnostic queue still consumes this observation and explicitly counts the loss.
    /// Call only after submit returns true; busy/failed calls create no new submission.
    pub fn trace_pending_submission(&mut self, sink: &SourceSubmissionSink, frame: Option<FrameWindow>) -> bool {
        let Some(pending) = &mut self.pending else {
            return false;
        };
        if pending.traced {
            return false;
        }
        let payload = match (&pending._input, &pending.head) {
            (Input::Software { .. }, Some(Input::Software { .. })) => {
                let [Some(tail), Some(head)] = pending.crossfade_uploads else {
                    return false;
                };
                SourceSubmissionPayload::SoftwareCrossfade([tail, head])
            }
            (Input::Hap { _frame: tail }, Some(Input::Hap { _frame: head })) => SourceSubmissionPayload::HapCrossfade {
                bytes: [tail.blocks().len() as u64, head.blocks().len() as u64],
            },
            (Input::Software { .. }, None) => {
                let Some(upload) = pending.software_upload else {
                    return false;
                };
                SourceSubmissionPayload::Software(upload)
            }
            (Input::Hap { _frame }, None) => SourceSubmissionPayload::Hap {
                bytes: _frame.blocks().len() as u64,
            },
            (Input::Decoded { .. }, None | Some(Input::Decoded { .. })) => SourceSubmissionPayload::Resident,
            #[cfg(test)]
            (Input::Texture { .. }, None) => SourceSubmissionPayload::Resident,
            _ => return false,
        };
        pending.traced = true;
        sink.record(SourceSubmissionTrace {
            stream: self.stream.id(),
            token: pending.token,
            descriptor: self.stream.descriptor(),
            frame: frame.map(Into::into),
            payload,
            observed_qpc: None,
        });
        true
    }
}
