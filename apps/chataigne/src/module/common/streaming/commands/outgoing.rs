use super::{preparation::PreparedCommand, ScriptSend, StreamingSendFrameKind};
use golden_io::RetainedPayload;
use std::ops::Deref;

#[derive(Clone, Copy)]
pub(crate) enum StreamSend<'a> {
    Prepared(&'a RetainedPayload<PreparedCommand>),
    Script(&'a ScriptSend),
}
impl<'a> StreamSend<'a> {
    pub fn input(self) -> SendInput<'a> {
        match self {
            Self::Prepared(request) => SendInput::Prepared(request),
            Self::Script(input) => SendInput::Script(input),
        }
    }
    pub fn frame_kind(self) -> StreamingSendFrameKind {
        match self {
            Self::Prepared(request) => request.request.frame_kind,
            Self::Script(input) => input.frame_kind(),
        }
    }
    pub fn description(self) -> &'a str {
        match self {
            Self::Prepared(request) => &request.request.description,
            Self::Script(_) => "script command",
        }
    }
}
impl<'a> From<&'a RetainedPayload<PreparedCommand>> for StreamSend<'a> {
    fn from(request: &'a RetainedPayload<PreparedCommand>) -> Self {
        Self::Prepared(request)
    }
}
impl<'a> From<&'a ScriptSend> for StreamSend<'a> {
    fn from(request: &'a ScriptSend) -> Self {
        Self::Script(request)
    }
}

pub(crate) enum SendInput<'a> {
    Prepared(&'a RetainedPayload<PreparedCommand>),
    #[cfg(test)]
    Borrowed(&'a [u8]),
    #[cfg(test)]
    Owned(Vec<u8>),
    Script(&'a ScriptSend),
}

/// Admission carries exactly one source. Failed preparation cannot expose an
/// empty successful command; ready canonical bytes never need actor-side copying.
#[derive(Debug)]
enum OutgoingData {
    Prepared(RetainedPayload<PreparedCommand>),
    Script(ScriptSend),
    Owned(Vec<u8>),
    Failed,
}

#[derive(Debug)]
pub(crate) struct OutgoingBytes {
    data: OutgoingData,
    limit: usize,
}
impl OutgoingBytes {
    pub fn new<'a>(input: impl Into<SendInput<'a>>, mut limit: usize) -> Result<Self, String> {
        let data = match input.into() {
            SendInput::Prepared(request) if request.request.bytes.len() <= limit => {
                OutgoingData::Prepared(request.clone())
            }
            SendInput::Script(script) => {
                limit = limit.min(script.output_bound());
                OutgoingData::Script(script.clone())
            }
            #[cfg(test)]
            SendInput::Borrowed(bytes) if bytes.len() <= limit => OutgoingData::Owned(bytes.to_vec()),
            #[cfg(test)]
            SendInput::Owned(bytes) if bytes.capacity() <= limit => OutgoingData::Owned(bytes),
            _ => {
                return Err("Stream command exceeds the transport retained-byte limit; command was not admitted".into())
            }
        };
        Ok(Self { data, limit })
    }
    pub fn weight(&self) -> usize {
        match &self.data {
            // Canonical storage (including spare capacity) has its own pool
            // reservation. Queues reserve logical output plus their own header.
            OutgoingData::Prepared(prepared) => prepared.request.bytes.len(),
            OutgoingData::Script(_) => self.limit,
            OutgoingData::Owned(bytes) => bytes.capacity(),
            OutgoingData::Failed => 0,
        }
    }
    pub fn encoded_len(&self) -> Option<usize> {
        match self.data {
            OutgoingData::Prepared(_) | OutgoingData::Owned(_) => Some(self.len()),
            OutgoingData::Script(_) | OutgoingData::Failed => None,
        }
    }
    pub fn prepare(&mut self) -> Result<(), String> {
        match &self.data {
            OutgoingData::Prepared(_) | OutgoingData::Owned(_) => return Ok(()),
            OutgoingData::Failed => {
                return Err("Stream script preparation previously failed; command must not be sent".into())
            }
            OutgoingData::Script(_) => {}
        }
        let OutgoingData::Script(script) = std::mem::replace(&mut self.data, OutgoingData::Failed) else {
            unreachable!("matched script preparation")
        };
        let mut request = script.materialize()?;
        request.bytes.shrink_to_fit();
        if request.bytes.capacity() > self.limit {
            return Err("Encoded stream script exceeds the transport output limit; command was not sent".into());
        }
        self.data = OutgoingData::Owned(request.bytes);
        Ok(())
    }
    pub fn into_vec(mut self) -> Result<Vec<u8>, String> {
        self.prepare()?;
        // Ownership-consuming frame/broadcast APIs copy on their native owner.
        // Slice-based writers keep the canonical bytes directly through Deref.
        Ok(match self.data {
            OutgoingData::Prepared(prepared) => prepared.request.bytes.clone(),
            OutgoingData::Owned(bytes) => bytes,
            OutgoingData::Script(_) | OutgoingData::Failed => unreachable!("prepared native output"),
        })
    }
}
impl Deref for OutgoingBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match &self.data {
            OutgoingData::Prepared(prepared) => &prepared.request.bytes,
            OutgoingData::Owned(bytes) => bytes,
            OutgoingData::Script(_) | OutgoingData::Failed => &[],
        }
    }
}

#[cfg(test)]
mod tests;
