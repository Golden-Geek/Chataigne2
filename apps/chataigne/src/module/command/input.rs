use super::{module_command_target, ModuleCommandExecuteEvent, MODULE_COMMAND_TARGET_MODULE_PATH};
use crate::app::module::common::json::retained_weight;
use golden_core::{events::CustomEvent, node::NodeId, parameter::ParamValue, process_ctx::ProcessTreeSnapshot};
use golden_io::RetainedPayload;
use std::{borrow::Cow, sync::Arc};

/// A frozen command view. Only bounded dispatch-target inspection runs on the
/// actor; override decoding and the snapshot overlay belong to the consuming worker.
#[derive(Clone, Debug)]
pub(crate) struct CapturedInvocation {
    pub snapshot: RetainedPayload<ProcessTreeSnapshot>,
    pub execution: Option<RetainedPayload<serde_json::Value>>,
    pub module: NodeId,
    pub command: NodeId,
    pub override_weight: usize,
}
impl CapturedInvocation {
    pub fn capture(
        snapshot: &Arc<ProcessTreeSnapshot>,
        command: NodeId,
        execution: Option<CustomEvent>,
        allowed_modules: &[&str],
    ) -> Result<Self, String> {
        if snapshot.node(command).is_none_or(|node| !node.enabled) {
            return Err("Command source is absent or disabled; command was not admitted".into());
        }
        let override_weight = execution
            .as_ref()
            .map(|event| retained_weight(&event.payload, 256 * 1024, 4096, 16))
            .transpose()?
            .unwrap_or(0);
        let target_parameter = snapshot.find_child_by_decl_id(command, MODULE_COMMAND_TARGET_MODULE_PATH);
        let target_override = target_parameter
            .and_then(|target| {
                execution
                    .as_ref()?
                    .payload
                    .get("param_overrides")?
                    .as_array()?
                    .iter()
                    .rev()
                    .find(|value| value.get("param_id").and_then(serde_json::Value::as_u64) == Some(target.0))
                    .and_then(|value| value.get("value"))
            })
            .map(|value| {
                retained_weight(value, 4096, 128, 8)?;
                serde_json::from_value::<ParamValue>(value.clone())
                    .map_err(|_| "Invalid command target override".to_owned())
            })
            .transpose()?;
        let module = module_command_target(snapshot, command, target_override.as_ref())?;
        if snapshot
            .node(module)
            .is_none_or(|node| !allowed_modules.contains(&node.node_type.as_str()))
        {
            return Err("Command target is not a supported module".into());
        }
        let execution = execution
            .map(|event| event.try_lease_payload(override_weight))
            .transpose()?;
        Ok(Self {
            snapshot: snapshot.try_lease().map_err(|error| error.to_string())?,
            execution,
            module,
            command,
            override_weight,
        })
    }

    pub fn view(&self) -> Result<Cow<'_, ProcessTreeSnapshot>, String> {
        let Some(execution) = &self.execution else {
            return Ok(Cow::Borrowed(&self.snapshot));
        };
        let execution: ModuleCommandExecuteEvent = serde_json::from_value((**execution).clone())
            .map_err(|error| format!("Invalid command invocation: {error}"))?;
        if execution.command_id != self.command {
            return Err("Invocation targets another command".into());
        }
        Ok(super::snapshot_with_command_overrides(
            &self.snapshot,
            execution.param_overrides,
        ))
    }
}
