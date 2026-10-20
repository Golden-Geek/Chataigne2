use crate::{AudioConfiguration, AudioDeviceSelection, DirectionConfiguration, storage::Budget};

impl AudioConfiguration {
    /// Conservative retained storage including spare capacities. Collection
    /// traversal is rejected before exceeding the caller's item budget.
    #[must_use]
    pub fn retained_bytes_within(&self, maximum_bytes: usize, maximum_items: usize) -> Option<usize> {
        let Self {
            enabled: _,
            input,
            output,
            physical_inputs,
            physical_outputs,
            virtual_inputs,
            virtual_outputs,
            input_patch,
            monitoring,
            playback_patch,
            output_patch,
            analysis_taps,
            master_gain: _,
        } = self;
        let mut budget = Budget {
            bytes: 0,
            maximum: maximum_bytes,
            items: maximum_items,
        };
        budget.items(1)?;
        budget.add(size_of::<Self>())?;
        for direction in [input, output] {
            let DirectionConfiguration {
                enabled: _,
                device,
                recovery_policy: _,
                buffer_policy: _,
            } = direction;
            if let Some(selection) = device {
                let AudioDeviceSelection {
                    target,
                    fallback_fingerprint,
                    last_known_label,
                    profile_key,
                } = selection;
                budget.target(target)?;
                budget.add(last_known_label.capacity())?;
                budget.add(profile_key.retained_heap_bytes())?;
                if let Some(fingerprint) = fallback_fingerprint {
                    budget.fingerprint(fingerprint)?;
                }
            }
        }
        for physical in [physical_inputs, physical_outputs] {
            budget.vector(physical)?;
            for key in physical {
                budget.add(key.retained_heap_bytes())?;
            }
        }
        budget.vector(virtual_inputs)?;
        for channel in virtual_inputs {
            budget.add(channel.label.capacity())?;
        }
        budget.vector(virtual_outputs)?;
        for channel in virtual_outputs {
            budget.add(channel.label.capacity())?;
        }
        budget.vector(input_patch)?;
        for route in input_patch {
            budget.add(route.source.retained_heap_bytes())?;
        }
        budget.vector(output_patch)?;
        for route in output_patch {
            budget.add(route.destination.retained_heap_bytes())?;
        }
        budget.vector(monitoring)?;
        budget.vector(playback_patch)?;
        budget.vector(analysis_taps)?;
        Some(budget.bytes)
    }
}

#[cfg(test)]
mod tests;
