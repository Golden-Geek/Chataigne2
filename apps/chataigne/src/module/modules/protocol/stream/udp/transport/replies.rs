use super::{StreamingWorkerEvent, UDP_EVENT_BYTES};
use crate::app::module::common::streaming::native_replies::NativeReply;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReplySource {
    Input,
    Output,
    Interfaces,
}

impl NativeReply for StreamingWorkerEvent {
    type Source = ReplySource;
    const SOURCES: usize = 3;
    fn weight(&self) -> usize {
        self.weight()
    }
    fn limit(source: ReplySource) -> usize {
        match source {
            ReplySource::Input => UDP_EVENT_BYTES,
            ReplySource::Output => 64 * 1024,
            ReplySource::Interfaces => 64 * 1024 + std::mem::size_of::<Self>(),
        }
    }
    fn oversized(source: ReplySource) -> Self {
        Self::Error(format!(
            "UDP {source:?} reply exceeds its {}-byte limit; result rejected and native work is not replayed",
            Self::limit(source),
        ))
    }
}
