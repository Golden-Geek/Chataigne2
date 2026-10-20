use super::*;
use golden_io::{PayloadPool, SharedPayload};
use std::mem::size_of;

pub(super) struct Selectors {
    pub connection: NodeId,
    pub driver: Option<BackendId>,
    pub input: String,
    pub output: String,
    pub input_parameter: NodeId,
    pub output_parameter: NodeId,
}

impl Selectors {
    pub(super) fn read(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Result<Self, ConfigurationError> {
        let connection = required_path(snapshot, module, "connection")?;
        let driver_value = child_enum(snapshot, connection, "audio_driver", NO_AUDIO_DRIVER);
        let duplex = driver_value == ASIO_AUDIO_DRIVER;
        let driver = if driver_value == NO_AUDIO_DRIVER {
            None
        } else {
            Some(BackendId::new(driver_value).map_err(|error| configuration_error(connection, error.to_string()))?)
        };
        let (input, output, input_parameter, output_parameter) = if duplex {
            let (parameter, value) = required_child_enum(snapshot, connection, "device")?;
            (value.clone(), value, parameter, parameter)
        } else {
            let (input_parameter, input) = required_child_enum(snapshot, connection, "input_device")?;
            let (output_parameter, output) = required_child_enum(snapshot, connection, "output_device")?;
            (input, output, input_parameter, output_parameter)
        };
        Ok(Self {
            connection,
            driver,
            input,
            output,
            input_parameter,
            output_parameter,
        })
    }
}

#[derive(Debug)]
pub(crate) struct SelectionPlan {
    targets: ProbeTargets,
    pub(crate) device_revision: u64,
    pub(crate) disable_before_probe: bool,
    pub(crate) pending: bool,
    pub(crate) input_physical: Vec<PhysicalChannelKey>,
    pub(crate) output_physical: Vec<PhysicalChannelKey>,
    pub(crate) bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ProbeTargets(SharedPayload<Vec<AudioDeviceTargetId>>);

impl SelectionPlan {
    pub(super) fn prepare(
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        inspector: &AudioDeviceInspectorState,
        revision: u64,
        previous: Option<&ProbeTargets>,
        pool: &PayloadPool,
    ) -> Result<Self, ConfigurationError> {
        let selectors = Selectors::read(snapshot, module)?;
        let targets = selected_probe_targets(selectors.driver.as_ref(), &selectors.input, &selectors.output)
            .map_err(|message| configuration_error(module, message))?;
        let disable_before_probe = requires_disable_before_probe(selectors.driver.as_ref(), inspector, &targets);
        let pending = probe_targets_are_pending(inspector, &targets);
        let input_physical = selected_physical_channels(
            selectors.driver.as_ref(),
            inspector,
            &selectors.input,
            AudioDirection::Input,
        );
        let output_physical = selected_physical_channels(
            selectors.driver.as_ref(),
            inspector,
            &selectors.output,
            AudioDirection::Output,
        );
        let target_bytes = target_storage(&targets);
        let targets = if let Some(previous) = previous.filter(|previous| previous.0.arc().as_ref() == &targets) {
            previous.clone()
        } else {
            let capacity = || ConfigurationError {
                node: module,
                retryable: true,
                message: "Sound Card probe-target lifetime capacity exhausted".into(),
            };
            ProbeTargets(
                pool.try_reserve(target_bytes)
                    .map_err(|_| capacity())?
                    .share_retained(targets, target_bytes)
                    .map_err(|_| capacity())?,
            )
        };
        let mut bytes = size_of::<Self>() + 2 * size_of::<usize>();
        for channels in [&input_physical, &output_physical] {
            bytes += channels.capacity() * size_of::<PhysicalChannelKey>();
            bytes += channels
                .iter()
                .map(PhysicalChannelKey::retained_heap_bytes)
                .sum::<usize>();
        }
        if bytes.saturating_add(target_storage(targets.0.arc())) > 16 * 1024 * 1024 {
            return Err(ConfigurationError {
                node: module,
                retryable: true,
                message: "Sound Card selection plan exceeds 16 MiB".into(),
            });
        }
        Ok(Self {
            targets,
            device_revision: revision,
            disable_before_probe,
            pending,
            input_physical,
            output_physical,
            bytes,
        })
    }

    pub(crate) fn interests(&self) -> ProbeTargets {
        self.targets.clone()
    }

    pub(crate) fn same_interests(&self, previous: &ProbeTargets) -> bool {
        std::sync::Arc::ptr_eq(self.targets.0.arc(), previous.0.arc())
    }

    pub(crate) fn submit(
        &self,
        control: &AudioControl,
    ) -> Result<golden_audio::CommandSequence, golden_audio::AudioError> {
        control.set_device_probe_interests(self.targets.0.arc().clone())
    }
}

fn target_storage(targets: &Vec<AudioDeviceTargetId>) -> usize {
    size_of::<Vec<AudioDeviceTargetId>>()
        + 2 * size_of::<usize>()
        + targets.capacity() * size_of::<AudioDeviceTargetId>()
        + targets
            .iter()
            .map(|target| match target {
                AudioDeviceTargetId::SystemDefault { backend } => backend.retained_heap_bytes(),
                AudioDeviceTargetId::Device { backend, device } => {
                    backend.retained_heap_bytes() + device.retained_heap_bytes()
                }
            })
            .sum::<usize>()
}

#[cfg(test)]
mod tests;
