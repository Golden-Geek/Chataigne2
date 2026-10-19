use crate::app::module::common::streaming::admission::retain_payload;
use golden_io::RetainedPayload;
use std::mem::size_of;

pub(super) const UDP_MAX_PAYLOAD: usize = 65_507;
pub(super) const UDP_COMMAND_BYTES: usize = 1024 * 1024;
pub(super) const UDP_HOST_BYTES: usize = 1024;

#[derive(Debug)]
pub(super) struct UdpOutputTarget {
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Debug)]
struct DestinationPlan {
    targets: Vec<UdpOutputTarget>,
    weight: usize,
}

/// Shared immutable destinations. The preparation pool retains the final owner so cached
/// plan replacement and rejected mailbox admission never destroy its strings on the actor.
#[derive(Clone, Debug)]
pub(super) struct PreparedOutputPlan(RetainedPayload<DestinationPlan>);

#[derive(Debug)]
pub(super) struct OutputPlanBuilder {
    targets: Vec<UdpOutputTarget>,
    weight: usize,
    valid: bool,
}

impl Default for OutputPlanBuilder {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
            weight: size_of::<DestinationPlan>(),
            valid: true,
        }
    }
}

impl OutputPlanBuilder {
    pub(super) fn push_target(&mut self, target: UdpOutputTarget) -> Result<(), String> {
        if !self.valid {
            return Err("UDP destination plan was rejected; command was not admitted".into());
        }
        self.valid = false;
        if target.remote_host.len() > UDP_HOST_BYTES {
            return Err("UDP destination host exceeds the 1024-byte limit; command was not admitted".into());
        }
        let old_capacity = self.targets.capacity();
        let capacity = if self.targets.len() == old_capacity {
            old_capacity.saturating_mul(2).max(4)
        } else {
            old_capacity
        };
        let weight = self
            .weight
            .saturating_add((capacity - old_capacity).saturating_mul(size_of::<UdpOutputTarget>()))
            .saturating_add(target.remote_host.capacity());
        if weight > UDP_COMMAND_BYTES {
            return Err("UDP destination plan exceeds the 1 MiB command limit; command was not admitted".into());
        }
        if capacity != old_capacity {
            self.targets.reserve_exact(capacity - self.targets.len());
        }
        self.weight =
            weight.saturating_add((self.targets.capacity() - capacity).saturating_mul(size_of::<UdpOutputTarget>()));
        if self.weight > UDP_COMMAND_BYTES {
            return Err("UDP destination allocation exceeds the 1 MiB command limit; command was not admitted".into());
        }
        self.targets.push(target);
        self.valid = true;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<PreparedOutputPlan, String> {
        if !self.valid {
            return Err("UDP destination plan was rejected; whole command was not admitted".into());
        }
        retain_payload(
            DestinationPlan {
                targets: self.targets,
                weight: self.weight,
            },
            self.weight,
        )
        .map(PreparedOutputPlan)
    }
}

impl PreparedOutputPlan {
    pub(super) fn len(&self) -> usize {
        self.0.targets.len()
    }
    pub(super) fn target(&self, index: usize) -> &UdpOutputTarget {
        &self.0.targets[index]
    }
}

/// One command owns one payload and shares its frozen destination plan. Admission
/// accepts the whole command before any destination can observe a datagram.
#[derive(Debug)]
pub(super) struct UdpOutboundSend {
    bytes: Vec<u8>,
    plan: PreparedOutputPlan,
    log_outgoing: bool,
    weight: usize,
}

impl UdpOutboundSend {
    pub(super) fn new(bytes: Vec<u8>, plan: PreparedOutputPlan, log_outgoing: bool) -> Result<Self, String> {
        if bytes.len() > UDP_MAX_PAYLOAD {
            return Err("UDP datagram exceeds the 65507-byte payload limit; command was not admitted".into());
        }
        if plan.len() == 0 {
            return Err("no enabled UDP outputs are configured; command was not admitted".into());
        }
        let weight = size_of::<Self>()
            .saturating_add(bytes.capacity())
            .saturating_add(plan.0.weight);
        if weight > UDP_COMMAND_BYTES {
            return Err("UDP command exceeds the 1 MiB retained-byte limit; command was not admitted".into());
        }
        Ok(Self {
            bytes,
            plan,
            log_outgoing,
            weight,
        })
    }

    pub(super) fn weight(&self) -> usize {
        self.weight
    }
    pub(super) fn len(&self) -> usize {
        self.plan.len()
    }
    pub(super) fn target(&self, index: usize) -> &UdpOutputTarget {
        self.plan.target(index)
    }
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(super) fn log_outgoing(&self) -> bool {
        self.log_outgoing
    }
}

#[cfg(test)]
mod tests;
