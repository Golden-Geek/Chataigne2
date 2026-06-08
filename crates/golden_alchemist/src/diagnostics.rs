use crate::{ANodeId, SocketId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagnosticOrigin {
    Graph,
    Node(ANodeId),
    Socket { node: ANodeId, socket: SocketId },
    Registry,
    Runtime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub origin: DiagnosticOrigin,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>, origin: DiagnosticOrigin) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: DiagnosticSeverity::Error,
            origin,
        }
    }
}
