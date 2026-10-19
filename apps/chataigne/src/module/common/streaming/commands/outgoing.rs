use super::{ScriptSend, StreamingSendFrameKind, StreamingSendRequest};
use std::ops::Deref;

#[derive(Clone, Copy)]
pub(crate) enum StreamSend<'a> {
    Prepared(&'a StreamingSendRequest),
    Script(&'a ScriptSend),
}
impl<'a> StreamSend<'a> {
    pub fn input(self) -> SendInput<'a> {
        match self {
            Self::Prepared(request) => SendInput::Borrowed(&request.bytes),
            Self::Script(input) => SendInput::Script(input),
        }
    }
    pub fn frame_kind(self) -> StreamingSendFrameKind {
        match self {
            Self::Prepared(request) => request.frame_kind,
            Self::Script(input) => input.frame_kind(),
        }
    }
    pub fn description(self) -> &'a str {
        match self {
            Self::Prepared(request) => &request.description,
            Self::Script(_) => "script command",
        }
    }
}
impl<'a> From<&'a StreamingSendRequest> for StreamSend<'a> {
    fn from(request: &'a StreamingSendRequest) -> Self {
        Self::Prepared(request)
    }
}
impl<'a> From<&'a ScriptSend> for StreamSend<'a> {
    fn from(request: &'a ScriptSend) -> Self {
        Self::Script(request)
    }
}

pub(crate) enum SendInput<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
    Script(&'a ScriptSend),
}
impl<'a> From<&'a [u8]> for SendInput<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::Borrowed(value)
    }
}
impl<'a, const N: usize> From<&'a [u8; N]> for SendInput<'a> {
    fn from(value: &'a [u8; N]) -> Self {
        Self::Borrowed(value)
    }
}
impl<'a> From<&'a Vec<u8>> for SendInput<'a> {
    fn from(value: &'a Vec<u8>) -> Self {
        Self::Borrowed(value)
    }
}
impl From<Vec<u8>> for SendInput<'_> {
    fn from(value: Vec<u8>) -> Self {
        Self::Owned(value)
    }
}

/// Created before native admission; only a native owner may prepare/take the bytes.
/// Script output reserves a conservative shape-derived bound, capped by the transport.
#[derive(Debug)]
pub(crate) struct OutgoingBytes {
    bytes: Vec<u8>,
    script: Option<ScriptSend>,
    limit: usize,
    failed: bool,
}
impl OutgoingBytes {
    pub fn new<'a>(input: impl Into<SendInput<'a>>, mut limit: usize) -> Result<Self, String> {
        let (bytes, script) = match input.into() {
            SendInput::Borrowed(bytes) if bytes.len() <= limit => (bytes.to_vec(), None),
            SendInput::Owned(bytes) if bytes.capacity() <= limit => (bytes, None),
            SendInput::Script(script) => {
                limit = limit.min(script.output_bound());
                (Vec::new(), Some(script.clone()))
            }
            _ => {
                return Err("Stream command exceeds the transport retained-byte limit; command was not admitted".into())
            }
        };
        Ok(Self {
            bytes,
            script,
            limit,
            failed: false,
        })
    }
    pub fn weight(&self) -> usize {
        if self.script.is_some() {
            self.limit
        } else {
            self.bytes.capacity()
        }
    }
    pub fn encoded_len(&self) -> Option<usize> {
        (self.script.is_none() && !self.failed).then_some(self.bytes.len())
    }
    pub fn prepare(&mut self) -> Result<(), String> {
        if self.failed {
            return Err("Stream script preparation previously failed; command must not be sent".into());
        }
        if let Some(script) = self.script.take() {
            self.failed = true;
            let mut request = script.materialize()?;
            request.bytes.shrink_to_fit();
            if request.bytes.capacity() > self.limit {
                return Err("Encoded stream script exceeds the transport output limit; command was not sent".into());
            }
            self.bytes = request.bytes;
            self.failed = false;
        }
        Ok(())
    }
    pub fn into_vec(mut self) -> Result<Vec<u8>, String> {
        self.prepare()?;
        Ok(self.bytes)
    }
}
impl Deref for OutgoingBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}
