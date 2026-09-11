use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Delivery and UI replay retention policy for one custom event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum CustomEventRetention {
    /// Preserve every event in order.
    #[default]
    Replay,
    /// Keep only the latest event for the same topic and origin.
    Latest,
    /// Deliver only through the engine inbox, without UI replay or transport exposure.
    Transient,
}
