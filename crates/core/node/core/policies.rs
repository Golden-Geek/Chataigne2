use serde::{Deserialize, Serialize};

use crate::script::ScriptHostPolicy;

/// Conversion helper for macro-generated script-host policy methods.
pub trait IntoScriptHostPolicyOption {
    /// Converts a value into an optional script-host policy.
    fn into_script_host_policy_option(self) -> Option<ScriptHostPolicy>;
}

impl IntoScriptHostPolicyOption for ScriptHostPolicy {
    fn into_script_host_policy_option(self) -> Option<ScriptHostPolicy> {
        Some(self)
    }
}

impl IntoScriptHostPolicyOption for Option<ScriptHostPolicy> {
    fn into_script_host_policy_option(self) -> Option<ScriptHostPolicy> {
        self
    }
}

/// User-context host policy for one node type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContextHostPolicy {
    /// Whether `UserContextNode` creation is enabled for this node.
    pub enabled: bool,
}

impl Default for UserContextHostPolicy {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl UserContextHostPolicy {
    /// Default policy used by `#[node(contextualizable)]` and `#[item(..., contextualizable)]`.
    pub fn default_contextualizable() -> Self {
        Self { enabled: true }
    }
}

/// Conversion helper for macro-generated user-context host policy methods.
pub trait IntoUserContextHostPolicyOption {
    /// Converts a value into an optional user-context host policy.
    fn into_user_context_host_policy_option(self) -> Option<UserContextHostPolicy>;
}

impl IntoUserContextHostPolicyOption for UserContextHostPolicy {
    fn into_user_context_host_policy_option(self) -> Option<UserContextHostPolicy> {
        Some(self)
    }
}

impl IntoUserContextHostPolicyOption for Option<UserContextHostPolicy> {
    fn into_user_context_host_policy_option(self) -> Option<UserContextHostPolicy> {
        self
    }
}
