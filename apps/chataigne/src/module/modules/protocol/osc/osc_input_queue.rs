use super::{
    osc_input::{InputDraft, InputMode, InputPreparer, INPUT_BATCH_MESSAGES},
    osc_message::OscDecodedMessage,
    osc_runtime::OscWorkerEvent,
};
use std::collections::VecDeque;

pub(super) struct InputQueue {
    decoded: VecDeque<Result<OscDecodedMessage, String>>,
    draft: Option<InputDraft>,
    preparer: Option<InputPreparer>,
}
impl InputQueue {
    pub(super) fn new(mode: InputMode) -> Result<Self, String> {
        let preparer = match mode {
            InputMode::Prepared(plan) => Some(InputPreparer::new(plan)?),
            #[cfg(test)]
            InputMode::Decoded => None,
        };
        Ok(Self {
            decoded: VecDeque::new(),
            draft: None,
            preparer,
        })
    }
    pub(super) fn is_empty(&self) -> bool {
        self.decoded.is_empty() && self.draft.is_none()
    }
    pub(super) fn push(&mut self, messages: Vec<Result<OscDecodedMessage, String>>) {
        assert!(self.is_empty(), "finish the admitted datagram before reading another");
        self.decoded.extend(messages);
    }
    pub(super) fn next(&mut self) -> Option<OscWorkerEvent> {
        if self.draft.is_none() {
            let message = match self.decoded.pop_front()? {
                Ok(message) => message,
                Err(error) => return Some(OscWorkerEvent::Error(error)),
            };
            if let Some(preparer) = &self.preparer {
                let mut messages = vec![message];
                while messages.len() < INPUT_BATCH_MESSAGES && self.decoded.front().is_some_and(Result::is_ok) {
                    messages.push(self.decoded.pop_front().expect("checked prefix").expect("valid prefix"));
                }
                match preparer.draft_batch(messages) {
                    Ok(draft) => self.draft = Some(draft),
                    Err(error) => return Some(OscWorkerEvent::Error(error)),
                }
            } else {
                #[cfg(test)]
                return Some(OscWorkerEvent::Message(message));
                #[cfg(not(test))]
                unreachable!("production input always has a preparer");
            }
        }
        let input = self.preparer.as_ref()?.retain(self.draft.as_mut()?)?;
        self.draft = None;
        Some(OscWorkerEvent::Input(input))
    }
}
