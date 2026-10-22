use super::super::{MqttModule, MqttTransportHandle};
use super::{Error, Key, Preparation};
use golden_core::{
    node::Node,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use std::sync::Arc;

impl MqttModule {
    pub(in super::super) fn notice_configuration(&mut self, snapshot: &ProcessTreeSnapshot) {
        let key = Key::capture(snapshot, self.id());
        if self.configuration_key != key {
            self.configuration_key = key;
            self.configuration = None;
            self.transport_dirty = self.module_enabled(snapshot);
            self.retry_at = None;
            self.retry_backoff.reset();
            self.request_service();
        }
    }

    pub(in super::super) fn configuration_needs_snapshot(&self) -> bool {
        self.transport_dirty && self.configuration.as_ref().is_none_or(Preparation::is_complete)
    }

    pub(in super::super) fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &Arc<ProcessTreeSnapshot>) {
        if !self.module_enabled(snapshot) {
            self.configuration = None;
            self.stop_transport();
            self.last_transport_config = None;
            self.transport_dirty = false;
            self.retry_at = None;
            self.clear_target_warning(ctx);
            self.base.set_connected(ctx, false);
            return;
        }
        self.notice_configuration(snapshot);
        if self
            .configuration
            .as_ref()
            .is_some_and(|request| !request.matches(snapshot, self.id()))
        {
            self.configuration = None;
        }
        if self.configuration.is_none() {
            let Some(wake) = &self.wake else { return };
            match Preparation::start(
                snapshot,
                self.id(),
                self.last_transport_config.clone(),
                wake.notifier.clone(),
            ) {
                Ok(request) => self.configuration = Some(request),
                Err(error) => {
                    self.configuration_failed(ctx, error);
                    return;
                }
            }
        }
        let preparation = self.configuration.as_mut().unwrap();
        let Some((config, unchanged)) = (match preparation.poll() {
            Ok(ready) => ready,
            Err(error) => {
                self.configuration_failed(ctx, error);
                return;
            }
        }) else {
            return;
        };

        self.retry_at = None;
        self.transport_dirty = false;
        if unchanged && self.transport.is_some() {
            self.configuration = None;
            self.retry_backoff.reset();
            return;
        }
        self.stop_transport();
        self.base.set_connected(ctx, false);
        let Some(wake) = &self.wake else { return };
        match MqttTransportHandle::spawn(config.clone(), self.id(), wake.notifier.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.configuration = None;
                self.retry_backoff.reset();
                self.clear_target_warning(ctx);
            }
            Err(error) => {
                // Keep the admitted ready result during native-slot recovery. A later
                // authored change cancels it before retry, without recopying its data.
                self.set_target_warning(ctx, &error);
                self.schedule_retry();
            }
        }
    }

    fn configuration_failed(&mut self, ctx: &mut ProcessCtx, error: Error) {
        self.configuration = None;
        self.transport_dirty = false;
        self.retry_at = None;
        self.set_target_warning(ctx, &error.message);
        if error.retryable {
            self.schedule_retry();
        } else {
            self.stop_transport();
            self.last_transport_config = None;
            self.base.set_connected(ctx, false);
        }
    }
}
