use super::*;
use golden_core::parameter::ParamValue;

pub(super) fn outbound_weight(message: &OscOutboundMessage) -> Result<usize, String> {
    if message.address.is_empty() || message.address.len() > OSC_MAX_ADDRESS {
        return Err("OSC address must contain 1-4096 bytes".into());
    }
    let mut weight = std::mem::size_of::<OscOutboundMessage>() + message.address.len();
    let values = match &message.payload {
        OscValuePayload::Single(value) => std::slice::from_ref(value),
        OscValuePayload::Multi(values) | OscValuePayload::Arguments(values) => {
            if values.len() > OSC_MAX_ARGUMENTS || values.capacity() > OSC_MAX_ARGUMENTS {
                return Err("OSC arguments exceed the 1024-value storage limit".into());
            }
            weight += values.capacity() * std::mem::size_of::<ParamValue>();
            values.as_slice()
        }
    };
    let expanded = !matches!(message.payload, OscValuePayload::Multi(_));
    let mut argument_slots = 0usize;
    for value in values {
        argument_slots += match (expanded, value) {
            (true, ParamValue::Trigger()) => 0,
            (true, ParamValue::Vec2(..)) => 2,
            (true, ParamValue::Vec3(..)) => 3,
            (true, ParamValue::Color(..)) => 4,
            _ => 1,
        };
        if argument_slots > OSC_MAX_ARGUMENTS {
            return Err("OSC expanded arguments exceed 1024 values".into());
        }
        if let ParamValue::Reference(reference) = value {
            if reference.relative_path_from_root.len() > 64 {
                return Err("OSC reference path exceeds 64 segments".into());
            }
        }
        weight = weight.saturating_add(value.retained_bytes());
        if weight > OSC_MAX_COMMAND_BYTES {
            return Err("OSC outgoing retained storage exceeds 128 KiB".into());
        }
    }
    if weight > OSC_MAX_COMMAND_BYTES {
        return Err("OSC outgoing retained storage exceeds 128 KiB".into());
    }
    Ok(weight)
}

pub(super) fn event_weight(event: &OscWorkerEvent) -> usize {
    std::mem::size_of::<OscWorkerEvent>()
        + match event {
            OscWorkerEvent::Ready(_) => 0,
            OscWorkerEvent::Error(error) | OscWorkerEvent::Failed(error) => error.capacity(),
            OscWorkerEvent::Message(message) => {
                message.address.capacity()
                    + match &message.payload {
                        OscValuePayload::Single(value) => value.retained_bytes(),
                        OscValuePayload::Multi(values) | OscValuePayload::Arguments(values) => {
                            values.capacity() * std::mem::size_of::<ParamValue>()
                                + values.iter().map(ParamValue::retained_bytes).sum::<usize>()
                        }
                    }
            }
        }
}

/// Admission guard for rosc's recursive bundle decoder and nested array drop.
/// It inspects framing and shape only; rosc remains the protocol codec.
pub(super) fn packet_shape(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.len() > OSC_MAX_DATAGRAM {
        return Err("OSC datagram exceeds 65507 bytes");
    }
    let mut pending = vec![(bytes, 0usize)];
    let mut packets = 0;
    while let Some((packet, depth)) = pending.pop() {
        packets += 1;
        if packets + pending.len() > 256 || depth > 32 {
            return Err("OSC bundle exceeds 256 packets or depth 32");
        }
        if packet.starts_with(b"#bundle\0") {
            if packet.len() < 16 {
                return Err("truncated OSC bundle");
            }
            let mut offset = 16;
            while offset < packet.len() {
                let size = packet.get(offset..offset + 4).ok_or("truncated OSC bundle size")?;
                let size = u32::from_be_bytes(size.try_into().unwrap()) as usize;
                offset += 4;
                let end = offset.checked_add(size).ok_or("OSC bundle size overflow")?;
                let child = packet
                    .get(offset..end)
                    .filter(|child| !child.is_empty())
                    .ok_or("truncated OSC bundle element")?;
                if packets + pending.len() >= 256 {
                    return Err("OSC bundle exceeds 256 packets");
                }
                pending.push((child, depth + 1));
                offset = end;
            }
        } else {
            let end = packet
                .iter()
                .position(|byte| *byte == 0)
                .ok_or("unterminated OSC address")?;
            if end > OSC_MAX_ADDRESS {
                return Err("OSC address exceeds 4096 bytes");
            }
            let tags = packet.get((end + 4) & !3..).ok_or("truncated OSC tags")?;
            let end = tags.iter().position(|byte| *byte == 0).ok_or("unterminated OSC tags")?;
            if end.saturating_sub(1) > OSC_MAX_ARGUMENTS {
                return Err("OSC type tags exceed 1024 entries");
            }
            let mut arrays = 0usize;
            for tag in &tags[..end] {
                match tag {
                    b'[' => {
                        arrays += 1;
                        if arrays > 32 {
                            return Err("OSC arrays exceed depth 32");
                        }
                    }
                    b']' => arrays = arrays.checked_sub(1).ok_or("unbalanced OSC array tags")?,
                    _ => {}
                }
            }
            if arrays != 0 {
                return Err("unbalanced OSC array tags");
            }
        }
    }
    Ok(())
}
