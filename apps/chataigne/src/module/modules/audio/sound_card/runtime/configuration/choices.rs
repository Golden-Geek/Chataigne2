use std::{
    mem::size_of,
    sync::{Mutex, OnceLock},
};

use golden_core::{
    edit::{EditPreconditions, NodeReadGuard},
    node::NodeId,
    parameter::{ParamValue, ParameterConstraints, ParameterEnumDomain, ParameterEnumOptions},
    process_ctx::{parameter_constraints_retained_bytes, ProcessTreeSnapshot},
};
use golden_io::PayloadPool;

use super::super::super::ASIO_AUDIO_DRIVER;
use super::super::{configuration_capabilities, find_path, observation::DeviceInventory, ConfigurationError};
use super::selection::Selectors;

pub(crate) mod options;
use options::{device_options_for_current, numeric_options, options_storage, DeviceChoice};

const MAX_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_PATCHES: usize = 4;

pub(super) fn option_pool() -> Result<PayloadPool, std::io::Error> {
    // Exported constraints can outlive the facade and UI replay window. Their
    // quotas must not consume the configuration worker's result tickets.
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("sound-card-enum-disposer", 512, 512 * 1024 * 1024)?);
    }
    Ok(pool.as_ref().unwrap().clone())
}

pub(crate) struct ChoicePatch {
    pub(crate) node: NodeId,
    pub(crate) constraints: ParameterConstraints,
}

/// Native construction owns option serialization, comparison and allocations.
/// The actor moves the fixed-size patch prefix once, then retains this admission
/// until the corresponding Core edits settle. Exported enum Arcs keep their own
/// canonical native lifetime quota even after this plan disappears.
pub(crate) struct ChoicePlan {
    publication: Mutex<Option<ChoicePublication>>,
    pub(crate) device_revision: u64,
    pub(crate) bytes: usize,
}

pub(crate) struct ChoicePublication {
    pub(crate) patches: Vec<ChoicePatch>,
    pub(crate) preconditions: EditPreconditions,
}

impl ChoicePlan {
    pub(super) fn validate_source(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Result<(), ConfigurationError> {
        for key in [
            "audio_driver",
            "device",
            "input_device",
            "output_device",
            "sample_rate",
            "buffer_size",
        ] {
            let node = find_path(snapshot, module, &format!("connection/{key}"));
            if node.is_none() && matches!(key, "device" | "input_device" | "output_device") {
                // Dependency-controlled selectors are absent while inactive;
                // Selectors::read validates the active device branch below.
                continue;
            }
            let value = node
                .and_then(|node| snapshot.node(node))
                .and_then(|node| node.param_value.as_ref());
            if !matches!(value, Some(ParamValue::Enum(value) | ParamValue::Str(value)) if value.len() <= 1024 * 1024) {
                return Err(ConfigurationError {
                    node: module,
                    retryable: false,
                    message: format!("Sound Card selector {key} must be enum text of at most 1 MiB"),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn prepare(
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        inventory: &DeviceInventory,
        pool: &PayloadPool,
    ) -> Result<Self, ConfigurationError> {
        Self::validate_source(snapshot, module)?;
        let selected = Selectors::read(snapshot, module)?;
        let error = |message: &str| ConfigurationError {
            node: module,
            message: message.into(),
            retryable: false,
        };
        let mut patches = Vec::with_capacity(MAX_PATCHES);
        // Guard the parent as well as present selectors: a formerly absent DSL
        // branch must not appear between preparation and application.
        let mut reads = Vec::with_capacity(8);
        // Presentation warning receipts are not choice inputs. Fencing all
        // module metadata would reject our own warning publication on retries.
        reads.push(
            NodeReadGuard::capture_identity(snapshot, module).ok_or_else(|| error("Sound Card module disappeared"))?,
        );
        for node in [selected.connection].into_iter().chain(
            [
                "audio_driver",
                "device",
                "input_device",
                "output_device",
                "sample_rate",
                "buffer_size",
            ]
            .into_iter()
            .filter_map(|key| find_path(snapshot, selected.connection, key)),
        ) {
            reads.push(
                NodeReadGuard::capture(snapshot, node).ok_or_else(|| error("Sound Card choice source disappeared"))?,
            );
        }
        let mut bytes = size_of::<Self>()
            + 2 * size_of::<usize>()
            + MAX_PATCHES * size_of::<ChoicePatch>()
            + reads.capacity() * size_of::<NodeReadGuard>()
            + size_of::<golden_io::RevisionGuard>()
            + inventory.guard().retained_heap_bytes();
        let mut add = |node: NodeId,
                       options: Vec<golden_core::parameter::ParameterEnumOption>|
         -> Result<(), ConfigurationError> {
            let previous = snapshot
                .node(node)
                .and_then(|state| state.param_constraints.as_ref())
                .ok_or_else(|| error("Sound Card selector constraints are absent"))?;
            let current = snapshot
                .node(node)
                .and_then(|state| state.param_value.as_ref())
                .and_then(|value| match value {
                    ParamValue::Enum(value) | ParamValue::Str(value) => Some(value),
                    _ => None,
                })
                .ok_or_else(|| error("Sound Card selector is not enum text"))?;
            if !options.iter().any(|option| option.variant_id == *current) {
                return Err(error("Sound Card prepared domain must preserve its authored selection"));
            }
            if previous.enum_options.as_slice() == options.as_slice() {
                return Ok(());
            }
            // Check before cloning nested constraint metadata. The source snapshot
            // has its own admission; new allocations belong to this plan's quota.
            if parameter_constraints_retained_bytes(previous) > MAX_BYTES {
                return Err(error("Sound Card selector constraints exceed 64 MiB"));
            }
            let weight = options_storage(&options);
            let capacity = || ConfigurationError {
                node: module,
                retryable: true,
                message: "Sound Card enum-domain lifetime capacity exhausted".into(),
            };
            let reservation = pool.try_reserve(weight).map_err(|_| capacity())?;
            let domain = ParameterEnumDomain::new(options);
            debug_assert_eq!(domain.retained_bytes(), weight);
            let retained = reservation.share_retained(domain, weight).map_err(|_| capacity())?;
            let mut constraints = previous.clone();
            constraints.enum_options = ParameterEnumOptions::from_shared(retained.arc().clone());
            bytes = bytes.saturating_add(parameter_constraints_retained_bytes(&constraints));
            if bytes > MAX_BYTES {
                return Err(error("Sound Card choice plan exceeds 64 MiB"));
            }
            patches.push(ChoicePatch { node, constraints });
            Ok(())
        };
        let device_choices = if selected
            .driver
            .as_ref()
            .is_some_and(|driver| driver.as_str() == ASIO_AUDIO_DRIVER)
        {
            [
                (
                    Some(selected.input_parameter),
                    selected.input.as_str(),
                    DeviceChoice::Duplex,
                ),
                (None, "", DeviceChoice::Output),
            ]
        } else {
            [
                (
                    Some(selected.input_parameter),
                    selected.input.as_str(),
                    DeviceChoice::Input,
                ),
                (
                    Some(selected.output_parameter),
                    selected.output.as_str(),
                    DeviceChoice::Output,
                ),
            ]
        };
        for (node, current, choice) in device_choices {
            if let Some(node) = node {
                let options = device_options_for_current(current, inventory.state(), choice, selected.driver.as_ref())
                    .map_err(error)?;
                add(node, options)?;
            }
        }
        let numeric = |key: &str| -> Result<(NodeId, &str), ConfigurationError> {
            let node = find_path(snapshot, selected.connection, key)
                .ok_or_else(|| error("Sound Card numeric selector is absent"))?;
            let value = snapshot
                .node(node)
                .and_then(|state| state.param_value.as_ref())
                .and_then(|value| match value {
                    ParamValue::Enum(value) | ParamValue::Str(value) => Some(value.as_str()),
                    _ => None,
                })
                .ok_or_else(|| error("Sound Card numeric selector is not an enum"))?;
            Ok((node, value))
        };
        let (rate_node, rate) = numeric("sample_rate")?;
        let (buffer_node, buffer) = numeric("buffer_size")?;
        let (rates, buffers) = configuration_capabilities(
            selected.driver.as_ref(),
            inventory.state(),
            &selected.input,
            &selected.output,
            rate,
            buffer,
        );
        add(rate_node, numeric_options(rate, "Hz", &rates).map_err(error)?)?;
        add(buffer_node, numeric_options(buffer, "frames", &buffers).map_err(error)?)?;
        Ok(Self {
            publication: Mutex::new(Some(ChoicePublication {
                patches,
                preconditions: EditPreconditions::new(reads, vec![inventory.guard()])
                    .expect("at most eight choice reads"),
            })),
            device_revision: inventory.revision(),
            bytes,
        })
    }

    pub(crate) fn take_publication(&self) -> Option<ChoicePublication> {
        self.publication
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }

    #[cfg(test)]
    pub(crate) fn take_patches(&self) -> Option<Vec<ChoicePatch>> {
        self.take_publication().map(|publication| publication.patches)
    }
}

#[cfg(test)]
mod tests;
