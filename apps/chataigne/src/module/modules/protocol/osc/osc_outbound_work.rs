use super::{
    osc_outbound::{Preparation, PreparedSend},
    osc_runtime::{worker::OscSocket, OscWorkerCommand},
};

enum Phase {
    Preparing(Box<Preparation>),
    Sending(Box<PreparedSend>),
    #[cfg(test)]
    Direct(super::osc_runtime::OscOutboundMessage),
}

pub(super) struct Current {
    phase: Option<Phase>,
    description: String,
}
impl Current {
    pub(super) fn new(command: OscWorkerCommand) -> Result<Self, String> {
        let description = command.description();
        let phase = match command {
            OscWorkerCommand::Captured(request) => Phase::Preparing(Box::new(Preparation::new(request)?)),
            #[cfg(test)]
            OscWorkerCommand::Direct(message) => Phase::Direct(message),
        };
        Ok(Self {
            phase: Some(phase),
            description,
        })
    }
    pub(super) fn description(&self) -> &str {
        &self.description
    }
    pub(super) fn complete(&self) -> bool {
        self.phase.is_none()
    }

    /// One bounded preparation turn or one destination effect. DNS/codec work is
    /// performed only here on the native worker, never by the admitting caller.
    pub(super) fn service(
        &mut self,
        socket: &mut impl OscSocket,
        cancelled: impl Fn() -> bool,
    ) -> Result<Option<Result<Option<String>, String>>, String> {
        if cancelled() {
            return Err("OSC transport cancelled before send".into());
        }
        let phase = self.phase.take().ok_or("OSC send has already completed")?;
        let mut prepared = match phase {
            Phase::Preparing(mut preparation) => {
                if !preparation.service()? {
                    self.phase = Some(Phase::Preparing(preparation));
                    return Ok(None);
                }
                Box::new(preparation.finish()?)
            }
            Phase::Sending(prepared) => prepared,
            #[cfg(test)]
            Phase::Direct(message) => {
                let packet = super::osc_message::encode_packet(&message.address, &message.payload)?;
                let bytes = rosc::encoder::encode(&packet).map_err(|error| error.to_string())?;
                if bytes.len() > 65_507 {
                    return Err("OSC encoded output exceeds 65507 bytes; native send was not attempted".into());
                }
                if cancelled() {
                    return Err("OSC transport cancelled before send".into());
                }
                return Ok(Some(effect(socket, &bytes, message.remote_address).map(|()| None)));
            }
        };
        if !prepared.request.is_live() {
            return Err("OSC source lifetime retired; remaining destinations were not sent".into());
        }
        let destination = &prepared.targets[prepared.next];
        let remote = socket.resolve(&destination.host, destination.port);
        // DNS may return after removal/disable or explicit cancellation. Recheck
        // both lifetimes before turning that result into an external effect.
        if cancelled() || !prepared.request.is_live() {
            return Err("OSC source or transport retired during destination resolution; output was not sent".into());
        }
        let outcome = remote
            .and_then(|remote| effect(socket, &prepared.bytes, remote))
            .map(|()| prepared.log(destination))
            .map_err(|error| {
                format!(
                    "OSC destination {}/{} failed: {error}; native effects are not replayed",
                    prepared.next + 1,
                    prepared.targets.len()
                )
            });
        prepared.next += 1;
        if prepared.next < prepared.targets.len() {
            self.phase = Some(Phase::Sending(prepared));
        }
        Ok(Some(outcome))
    }
}

fn effect(socket: &mut impl OscSocket, bytes: &[u8], remote: std::net::SocketAddr) -> Result<(), String> {
    match socket.send(bytes, remote) {
        Ok(length) if length == bytes.len() => Ok(()),
        Ok(_) => Err("OSC native send reported a partial datagram; effect is unconfirmed".into()),
        Err(error) => Err(format!("OSC native send failed: {error}")),
    }
}
