use super::*;
use crate::app::MidiSendPacket;
use golden_core::engine::NodeWaker;
use golden_io::RetainedPayload;
use std::{cmp::Ordering as CmpOrdering, collections::BinaryHeap};

pub(super) struct Batch {
    at: Instant,
    order: u64,
    packets: Vec<MidiSendPacket>,
    next: usize,
    origin: Instant,
    producer: Option<NodeWaker>,
    begun: bool,
    log: bool,
    // This lease is dropped after all charged packet and scheduler storage.
    _admission: RetainedPayload<RetirementPermit>,
}
impl Batch {
    pub(super) fn prepare(admitted: AdmittedRequest, order: u64) -> Result<Self, String> {
        let PreparedOutput {
            mut request,
            producer,
            captured_at,
            log,
        } = admitted.request.prepare()?;
        if request.packets.is_empty() || request.packets.capacity() > 2 {
            return Err("MIDI request must contain one or two packets".into());
        }
        let mut weight = 4096usize;
        for packet in &request.packets {
            if packet.delay_ms > i32::MAX as u64 {
                return Err("MIDI delay exceeds its authored 2147483647 ms limit".into());
            }
            if packet.bytes.capacity() > MESSAGE_BYTES || packet.description.capacity() > 512 {
                return Err("MIDI packet exceeds 64 KiB / 512-byte description".into());
            }
            captured_at
                .checked_add(Duration::from_millis(packet.delay_ms))
                .ok_or("MIDI delayed deadline is not representable")?;
            weight = weight.saturating_add(packet.bytes.capacity() + packet.description.capacity());
        }
        request.packets.sort_by_key(|packet| packet.delay_ms);
        let at = captured_at
            .checked_add(Duration::from_millis(request.packets[0].delay_ms))
            .expect("validated deadline");
        let admission = admitted
            .reservation
            .retain(admitted.local, weight)
            .map_err(|_| "MIDI prepared plan exceeds its admitted reservation")?;
        Ok(Self {
            at,
            order,
            packets: request.packets,
            next: 0,
            origin: captured_at,
            producer,
            begun: false,
            log,
            _admission: admission,
        })
    }
}
impl PartialEq for Batch {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.order) == (other.at, other.order)
    }
}
impl Eq for Batch {}
impl PartialOrd for Batch {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}
impl Ord for Batch {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        (other.at, other.order).cmp(&(self.at, self.order))
    }
}

#[derive(Default)]
pub(super) struct Schedule(BinaryHeap<Batch>);
impl Schedule {
    pub(super) fn push(&mut self, batch: Batch) {
        self.0.push(batch);
    }
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.0.peek().map(|batch| batch.at)
    }
    pub(super) fn service<G: OutputConnection>(
        &mut self,
        connection: &mut G,
        failed: bool,
    ) -> Option<(MidiOutputEvent, bool)> {
        if !failed && self.deadline().is_none_or(|at| at > Instant::now()) {
            return None;
        }
        let mut batch = self.0.pop()?;
        if failed {
            return Some((
                MidiOutputEvent::Error("accepted MIDI batch suffix was not sent after output failure".into()),
                false,
            ));
        }
        // Once a note-on succeeds, source deletion must not discard its accepted
        // note-off. Explicit module/transport cancellation still owns that suffix.
        if !batch.begun && batch.producer.as_ref().is_some_and(NodeWaker::is_closed) {
            return Some((
                MidiOutputEvent::Error("MIDI source retired before its first effect; complete batch cancelled".into()),
                false,
            ));
        }
        let packet = &batch.packets[batch.next];
        let event = match connection.send(&packet.bytes) {
            Ok(()) => {
                batch.begun = true;
                let log = batch.log.then(|| {
                    let shown = &packet.bytes[..packet.bytes.len().min(256)];
                    format!(
                        "Sent MIDI {} ({}{}); {} bytes",
                        packet.description,
                        format_midi_bytes(shown),
                        if shown.len() < packet.bytes.len() { " ..." } else { "" },
                        packet.bytes.len()
                    )
                });
                MidiOutputEvent::Sent(log)
            }
            Err(error) => MidiOutputEvent::Error(diagnostic(format!(
                "{}; native outcome unconfirmed, packet is not replayed",
                diagnostic(error)
            ))),
        };
        let native_failed = matches!(event, MidiOutputEvent::Error(_));
        batch.next += 1;
        if batch.next < batch.packets.len() {
            batch.at = batch
                .origin
                .checked_add(Duration::from_millis(batch.packets[batch.next].delay_ms))
                .expect("validated deadline");
            self.0.push(batch);
        }
        Some((event, native_failed))
    }
}
