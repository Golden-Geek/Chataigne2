use super::output_plan::{OutputPreparation, PlanTicket, TICKET_HEAP_BYTES};
use crate::app::module::common::streaming::admission::retain_payload;
use crate::app::module::common::streaming::commands::{OutgoingBytes, SendInput};
use golden_core::{node::NodeId, process_ctx::ProcessTreeSnapshot};
use golden_io::RetainedPayload;
use std::mem::size_of;
use std::sync::Arc;

pub(super) const UDP_MAX_PAYLOAD: usize = 65_507;
pub(super) const UDP_COMMAND_BYTES: usize = 1024 * 1024;
pub(super) const UDP_HOST_BYTES: usize = 1024;
pub(super) const UDP_PLAN_BYTES: usize =
    UDP_COMMAND_BYTES - 128 * 1024 - size_of::<UdpOutboundSend>() - TICKET_HEAP_BYTES;

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
    limit: usize,
}

impl Default for OutputPlanBuilder {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
            weight: size_of::<DestinationPlan>(),
            valid: true,
            limit: UDP_PLAN_BYTES,
        }
    }
}

impl OutputPlanBuilder {
    pub(super) fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.min(UDP_PLAN_BYTES),
            ..Self::default()
        }
    }
    pub(super) fn push_target(&mut self, target: UdpOutputTarget) -> Result<(), String> {
        if !self.valid {
            return Err("UDP destination plan was rejected; no output was sent".into());
        }
        self.valid = false;
        if target.remote_host.len() > UDP_HOST_BYTES {
            return Err("UDP destination host exceeds the 1024-byte limit; no output was sent".into());
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
        if weight > self.limit {
            return Err("UDP destination plan exceeds the 1 MiB command limit; no output was sent".into());
        }
        if capacity != old_capacity {
            self.targets.reserve_exact(capacity - self.targets.len());
        }
        self.weight =
            weight.saturating_add((self.targets.capacity() - capacity).saturating_mul(size_of::<UdpOutputTarget>()));
        if self.weight > self.limit {
            return Err("UDP destination allocation exceeds the 1 MiB command limit; no output was sent".into());
        }
        self.targets.push(target);
        self.valid = true;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<PreparedOutputPlan, String> {
        if !self.valid {
            return Err("UDP destination plan was rejected; no output was sent".into());
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
    pub(super) fn weight(&self) -> usize {
        self.0.weight
    }
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
    bytes: OutgoingBytes,
    ticket: PlanTicket,
    plan: Option<PreparedOutputPlan>,
    cold: Option<OutputPreparation>,
    log_outgoing: bool,
    weight: usize,
}

impl UdpOutboundSend {
    #[cfg(test)]
    pub(super) fn new(bytes: Vec<u8>, plan: PreparedOutputPlan, log_outgoing: bool) -> Result<Self, String> {
        Self::with_ticket(bytes, PlanTicket::ready(plan), None, log_outgoing)
    }

    pub(super) fn with_ticket<'a>(
        bytes: impl Into<SendInput<'a>>,
        ticket: PlanTicket,
        snapshot: Option<(&Arc<ProcessTreeSnapshot>, NodeId)>,
        log_outgoing: bool,
    ) -> Result<Self, String> {
        let bytes = OutgoingBytes::new(bytes, UDP_MAX_PAYLOAD)?;
        let plan = ticket
            .result()
            .map(|result| result.as_ref().cloned().map_err(|error| error.message.clone()))
            .transpose()?;
        if plan.as_ref().is_some_and(|plan| plan.len() == 0) {
            return Err("no enabled UDP outputs are configured; command was not admitted".into());
        }
        let weight = size_of::<Self>()
            .saturating_add(bytes.weight())
            .saturating_add(TICKET_HEAP_BYTES)
            .saturating_add(plan.as_ref().map_or(ticket.limit(), PreparedOutputPlan::weight));
        if weight > UDP_COMMAND_BYTES {
            return Err("UDP command exceeds the 1 MiB retained-byte limit; command was not admitted".into());
        }
        let cold = snapshot
            .map(|(snapshot, root)| {
                snapshot
                    .try_lease()
                    .map(|lease| OutputPreparation::new(lease, root, ticket.limit()))
                    .map_err(|error| error.to_string())
            })
            .transpose()?;
        Ok(Self {
            bytes,
            ticket,
            plan,
            cold,
            log_outgoing,
            weight,
        })
    }

    pub(super) fn prepare_turn(&mut self, cancelled: impl Fn() -> bool) -> Result<bool, String> {
        if let Some(cold) = self.cold.as_mut() {
            let Some(result) = cold.service(&cancelled) else {
                return Ok(false);
            };
            self.ticket.complete(result);
            self.cold = None;
        }
        if self.plan.is_none() {
            self.plan = Some(
                self.ticket
                    .result()
                    .ok_or("UDP output plan has no completed native owner")?
                    .as_ref()
                    .cloned()
                    .map_err(|error| error.message.clone())?,
            );
        }
        if self.len() == 0 {
            return Err("no enabled UDP outputs are configured; no output was sent".into());
        }
        // Seal the shared destination ticket independently of this invocation's
        // bytes. A malformed script must not poison later commands using the plan.
        self.bytes.prepare()?;
        Ok(true)
    }

    #[cfg(test)]
    pub(super) fn prepare(&mut self, cancelled: impl Fn() -> bool) -> Result<(), String> {
        while !self.prepare_turn(&cancelled)? {}
        Ok(())
    }

    pub(super) fn fail_preparation(&mut self, error: String) {
        if self.cold.take().is_some() {
            self.ticket.fail(error);
        }
    }

    pub(super) fn prepared_count(&self) -> Option<usize> {
        self.plan.as_ref().map(PreparedOutputPlan::len)
    }

    pub(super) fn weight(&self) -> usize {
        self.weight
    }
    pub(super) fn len(&self) -> usize {
        self.plan.as_ref().map_or(0, PreparedOutputPlan::len)
    }
    pub(super) fn target(&self, index: usize) -> &UdpOutputTarget {
        self.plan.as_ref().expect("prepared UDP send cursor").target(index)
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
