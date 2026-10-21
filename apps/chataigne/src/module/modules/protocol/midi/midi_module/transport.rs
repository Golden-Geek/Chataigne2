use super::*;

impl MidiModule {
    pub(super) fn port_refresh_due(&self) -> bool {
        self.port_refresh_elapsed >= MIDI_PORT_REFRESH_INTERVAL_SECS
    }

    pub(super) fn refresh_port_options(&mut self, ctx: &mut ProcessCtx) {
        if !self.node_data().effective_enabled {
            self.discovery = None;
            return;
        }
        if self.discovery.is_none() {
            match MidiDiscovery::spawn(Self::native_notifier(ctx)) {
                Ok(discovery) => self.discovery = Some(discovery),
                Err(error) => {
                    self.set_port_options_warning(ctx, &error);
                    return;
                }
            }
        }
        let (event, stopped) = self.discovery.as_ref().unwrap().take();
        if stopped {
            self.discovery = None;
        }
        let event = match event {
            Some(event) => event,
            None if stopped => Err("MIDI discovery worker stopped; retrying".into()),
            None => return,
        };
        match event {
            Ok(options) => {
                if self.input_port.is_bound() {
                    sync_midi_port_enum_options(ctx, self.input_port.id(), midi_input_port_options(&options.inputs));
                    self.input_port.clear_warning(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID));
                }
                if self.output_port.is_bound() {
                    sync_midi_port_enum_options(ctx, self.output_port.id(), midi_output_port_options(&options.outputs));
                    self.output_port.clear_warning(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID));
                }
            }
            Err(error) => {
                self.set_port_options_warning(ctx, &error);
            }
        }
    }

    fn set_port_options_warning(&self, ctx: &mut ProcessCtx, error: &str) {
        if self.input_port.is_bound() {
            self.input_port
                .set_warning_with(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID), error, None);
        }
        if self.output_port.is_bound() {
            self.output_port
                .set_warning_with(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID), error, None);
        }
    }

    pub(super) fn refresh_input(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.input_dirty = false;

        if self.input_port.get_ref().as_str().len() > 1024 {
            self.stop_input();
            self.set_input_warning(ctx, "MIDI input selection exceeds 1024 bytes");
            self.refresh_connected(ctx);
            return;
        }

        if !self.module_enabled(snapshot) {
            self.stop_input();
            self.last_input_config = None;
            self.clear_input_warning(ctx);
            self.refresh_connected(ctx);
            return;
        }

        let Some(config) = self.input_config() else {
            self.stop_input();
            self.last_input_config = None;
            self.clear_input_warning(ctx);
            self.refresh_connected(ctx);
            return;
        };

        if self.input.is_some() && self.last_input_config.as_ref() == Some(&config) {
            self.refresh_connected(ctx);
            return;
        }

        self.stop_input();
        match MidiInputHandle::spawn(config.clone(), Self::native_notifier(ctx)) {
            Ok(handle) => {
                self.input = Some(handle);
                self.last_input_config = Some(config);
                self.clear_input_warning(ctx);
            }
            Err(error) => {
                logerror!("Failed to connect MIDI input: {}", error);
                self.last_input_config = None;
                self.set_input_warning(ctx, error.as_str());
            }
        }
        self.refresh_connected(ctx);
    }

    pub(super) fn refresh_output(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.output_dirty = false;

        if self.output_port.get_ref().as_str().len() > 1024 {
            self.stop_output();
            self.set_output_warning(ctx, "MIDI output selection exceeds 1024 bytes");
            self.refresh_connected(ctx);
            return;
        }

        if !self.module_enabled(snapshot) {
            self.stop_output();
            self.last_output_config = None;
            self.clear_output_warning(ctx);
            self.refresh_connected(ctx);
            return;
        }

        let Some(config) = self.output_config() else {
            self.stop_output();
            self.last_output_config = None;
            self.clear_output_warning(ctx);
            self.refresh_connected(ctx);
            return;
        };

        if self.output.is_some() && self.last_output_config.as_ref() == Some(&config) {
            self.refresh_connected(ctx);
            return;
        }

        if self.retiring_output.is_some() {
            return;
        }
        if let Some(mut previous) = self.output.take() {
            previous.finish_admission();
            self.retiring_output = Some(previous);
            self.refresh_connected(ctx);
            return;
        }
        match MidiOutputHandle::spawn(config.clone(), Self::native_notifier(ctx)) {
            Ok(handle) => {
                self.output = Some(handle);
                self.last_output_config = Some(config);
                self.clear_output_warning(ctx);
            }
            Err(error) => {
                logerror!("Failed to connect MIDI output: {}", error);
                self.last_output_config = None;
                self.set_output_warning(ctx, error.as_str());
            }
        }
        self.refresh_connected(ctx);
    }

    pub(super) fn input_config(&self) -> Option<MidiInputConfig> {
        midi_port_selected(self.input_port.get_ref().as_str()).then(|| MidiInputConfig {
            port_variant: self.input_port.get_ref().as_str().to_string(),
        })
    }

    pub(super) fn output_config(&self) -> Option<MidiOutputConfig> {
        midi_port_selected(self.output_port.get_ref().as_str()).then(|| MidiOutputConfig {
            port_variant: self.output_port.get_ref().as_str().to_string(),
        })
    }

    pub(super) fn refresh_connected(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_connected(
            ctx,
            midi_transport_connected(
                midi_port_selected(self.input_port.get_ref().as_str()),
                self.input.as_ref().is_some_and(MidiInputHandle::is_ready),
                midi_port_selected(self.output_port.get_ref().as_str()),
                self.output.as_ref().is_some_and(MidiOutputHandle::is_ready),
            ),
        );
    }

    pub(super) fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                midi_port_selected(self.input_port.get_ref().as_str()),
                midi_port_selected(self.output_port.get_ref().as_str()),
            ),
        );
    }

    pub(super) fn stop_input(&mut self) {
        if let Some(mut input) = self.input.take() {
            input.stop();
        }
    }

    pub(super) fn stop_output(&mut self) {
        self.output = None;
        self.retiring_output = None;
    }

    pub(super) fn drain_input_events(&mut self, ctx: &mut ProcessCtx) {
        let mut events = Vec::new();
        // Retain one bounded actor batch while graph materialization catches up.
        if !self.pending_incoming_messages.is_empty() {
            return;
        }
        let mut stopped = false;
        if let Some(input) = &self.input {
            for _ in 0..32 {
                match input.try_recv() {
                    Ok(event) => events.push(event),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        stopped = true;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if stopped {
            self.input = None;
            self.last_input_config = None;
            self.set_input_warning(ctx, "MIDI input worker stopped; reconnecting");
        }
        self.refresh_connected(ctx);

        let mut received = false;
        for event in events {
            match event {
                MidiInputEvent::Ready => self.clear_input_warning(ctx),
                MidiInputEvent::Error(error) => self.set_input_warning(ctx, &error),
                MidiInputEvent::Rejected(count) => self.set_input_warning(ctx, &format!("MIDI input rejected {count} message(s) before admission: callback capacity or 64 KiB message limit exceeded")),
                MidiInputEvent::Message {
                    bytes,
                    message,
                    received_at,
                    clock_timing,
                } => {
                    received = true;
                    self.log_incoming_message(&message, bytes.as_slice());
                    self.pending_incoming_messages.push(PendingIncomingMessage {
                        message,
                        received_at: Some(received_at),
                        clock_timing,
                    });
                }
                MidiInputEvent::UnsupportedMessage { bytes, .. } => {
                    logerror!(
                        "Ignored unsupported MIDI message {}",
                        format_midi_bytes(bytes.as_slice())
                    );
                }
            }
        }

        if received {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    pub(super) fn native_notifier(ctx: &ProcessCtx) -> golden_io::WorkerNotifier {
        let waker = ctx.node_waker().ok();
        golden_io::WorkerNotifier::new(move || {
            if let Some(waker) = &waker {
                let _ = waker.wake();
            }
        })
    }

    pub(super) fn drain_output_events(&mut self, ctx: &mut ProcessCtx) {
        let mut events = Vec::with_capacity(64);
        let mut stopped = false;
        if let Some(output) = &self.output {
            stopped = output.drain(&mut events).state == golden_io::PendingDrainState::Disconnected;
        }
        if stopped {
            self.output = None;
            self.last_output_config = None;
            self.set_output_warning(ctx, "MIDI output worker stopped; reconnecting");
        }
        let retired = self
            .retiring_output
            .as_ref()
            .is_some_and(|output| output.drain(&mut events).state == golden_io::PendingDrainState::Disconnected);
        if retired {
            self.retiring_output = None;
            self.output_dirty = true;
        }
        for event in events {
            match event {
                MidiOutputEvent::Ready => self.clear_output_warning(ctx),
                MidiOutputEvent::Sent(log) => {
                    self.clear_output_warning(ctx);
                    self.base.emit_outgoing_traffic(ctx);
                    if let Some(log) = log {
                        golden_core::log!(origin = self.id(); log);
                    }
                }
                MidiOutputEvent::Error(error) => self.set_output_warning(ctx, &error),
            }
        }
        self.refresh_connected(ctx);
    }
}
