use crate::ui_sync::{UiAck, UiHistoryState};

/// Authoritative rejection returned by public graph-edit and project-transaction facades.
///
/// The complete acknowledgement is retained so non-UI callers observe the same error code,
/// message, and post-operation history state as UI and transport callers.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphEditError {
    acknowledgement: Box<UiAck>,
}

impl GraphEditError {
    pub(super) fn from_rejected_ack(acknowledgement: UiAck) -> Self {
        debug_assert!(!acknowledgement.success);
        Self {
            acknowledgement: Box::new(acknowledgement),
        }
    }

    /// Returns the stable rejection code supplied by the authoritative engine acknowledgement.
    pub fn code(&self) -> Option<&str> {
        self.acknowledgement.error_code.as_deref()
    }

    /// Returns the actionable rejection message supplied by the authoritative engine.
    pub fn message(&self) -> Option<&str> {
        self.acknowledgement.error_message.as_deref()
    }

    /// Returns history state captured in the same actor turn as the rejected operation.
    pub fn history(&self) -> &UiHistoryState {
        &self.acknowledgement.history
    }

    /// Returns the complete rejected acknowledgement used by UI and transport consumers.
    pub fn acknowledgement(&self) -> &UiAck {
        &self.acknowledgement
    }

    /// Consumes this error and returns its complete rejected acknowledgement.
    pub fn into_acknowledgement(self) -> UiAck {
        *self.acknowledgement
    }
}

impl std::fmt::Display for GraphEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = self.message() {
            formatter.write_str(message)
        } else if let Some(code) = self.code() {
            formatter.write_str(code)
        } else {
            formatter.write_str("graph edit rejected")
        }
    }
}

impl std::error::Error for GraphEditError {}

pub(super) fn transaction_acknowledgement_result(acknowledgement: UiAck) -> Result<UiAck, GraphEditError> {
    if acknowledgement.success {
        Ok(acknowledgement)
    } else {
        Err(GraphEditError::from_rejected_ack(acknowledgement))
    }
}

pub(super) fn graph_revision_result(acknowledgement: UiAck) -> Result<UiHistoryState, GraphEditError> {
    if acknowledgement.success {
        Ok(acknowledgement.history)
    } else {
        Err(GraphEditError::from_rejected_ack(acknowledgement))
    }
}
