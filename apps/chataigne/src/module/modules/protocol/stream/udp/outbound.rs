use std::mem::size_of;

pub(super) const UDP_MAX_PAYLOAD: usize = 65_507;
pub(super) const UDP_COMMAND_BYTES: usize = 1024 * 1024;
const UDP_HOST_BYTES: usize = 1024;

#[derive(Debug)]
pub(super) struct UdpOutputTarget {
    pub remote_host: String,
    pub remote_port: u16,
}

/// One command owns one payload and its frozen, ordered destinations. Native admission
/// accepts the whole command before any destination can observe a datagram.
#[derive(Debug)]
pub(super) struct UdpOutboundSend {
    bytes: Vec<u8>,
    targets: Vec<UdpOutputTarget>,
    log_outgoing: bool,
    weight: usize,
    valid: bool,
}

impl UdpOutboundSend {
    pub(super) fn new(bytes: Vec<u8>, log_outgoing: bool) -> Result<Self, String> {
        if bytes.len() > UDP_MAX_PAYLOAD {
            return Err("UDP datagram exceeds the 65507-byte payload limit; command was not admitted".into());
        }
        let weight = size_of::<Self>().saturating_add(bytes.capacity());
        if weight > UDP_COMMAND_BYTES {
            return Err("UDP command exceeds the 1 MiB retained-byte limit; command was not admitted".into());
        }
        Ok(Self {
            bytes,
            targets: Vec::new(),
            log_outgoing,
            weight,
            valid: true,
        })
    }

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

    pub(super) fn admission_weight(&self) -> Result<usize, String> {
        if !self.valid || self.targets.is_empty() {
            return Err("UDP command has no valid destination plan; whole command was not admitted".into());
        }
        Ok(self.weight)
    }
    pub(super) fn len(&self) -> usize {
        self.targets.len()
    }
    pub(super) fn target(&self, index: usize) -> &UdpOutputTarget {
        &self.targets[index]
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
