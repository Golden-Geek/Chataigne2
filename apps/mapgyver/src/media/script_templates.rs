//! App-owned source templates are embedded so installed scripts need no repository files.
use golden_core::script::{ScriptNodeConfig, ScriptSource};

pub(super) fn config() -> ScriptNodeConfig {
    ScriptNodeConfig {
        source: ScriptSource::Inline {
            text: [
                include_str!("script_templates/source_api.js"),
                include_str!("script_templates/callbacks.js"),
            ]
            .join("\n"),
        },
    }
}
