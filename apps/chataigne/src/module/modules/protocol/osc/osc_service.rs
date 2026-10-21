use super::{
    osc_catalog_delivery::CatalogDelivery,
    osc_discovery::{Discovery, Selection},
    *,
};
use crate::app::module::common::worker_wake::ModuleWake;

impl OscModuleBase {
    pub(super) fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }

    pub(super) fn register_service(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let source = ctx.node_waker().map_err(|error| error.to_string())?;
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.stop_transport();
            self.discovery = None;
            self.pending_catalog = None;
            self.pending_outbound_nodes.clear();
            self.outbound_retry_at = None;
            self.discovery_retry_at = None;
            self.wake = Some(ModuleWake::new(source)?);
        }
        self.transport_dirty = true;
        self.request_service();
        Ok(())
    }

    fn service_discovery(&mut self, ctx: &mut ProcessCtx) {
        let selection = match Selection::parse(self.network_interface.get_ref().as_str()) {
            Ok(selection) => selection,
            Err(error) => {
                self.discovery = None;
                self.pending_catalog = None;
                self.network_interface
                    .set_warning_with(ctx, Some(NETWORK_INTERFACE_WARNING_ID), &error, None);
                return;
            }
        };
        if self.discovery.is_none() && self.discovery_retry_at.is_none_or(|at| Instant::now() >= at) {
            let Some(wake) = &self.wake else { return };
            match Discovery::spawn(selection, wake.notifier.clone()) {
                Ok(discovery) => {
                    self.discovery = Some(discovery);
                    self.discovery_retry_at = None;
                }
                Err(error) => {
                    self.discovery_retry_at = Some(Instant::now() + Duration::from_secs(1));
                    self.network_interface
                        .set_warning_with(ctx, Some(NETWORK_INTERFACE_WARNING_ID), &error, None);
                }
            }
        }
        if let Some(discovery) = &self.discovery {
            if let Err(error) = discovery.select(selection) {
                self.network_interface
                    .set_warning_with(ctx, Some(NETWORK_INTERFACE_WARNING_ID), &error, None);
            }
            if self
                .pending_catalog
                .as_ref()
                .is_some_and(|delivery| delivery.event.generation != discovery.generation())
            {
                self.pending_catalog = None;
            }
            if self.pending_catalog.is_none() {
                let (event, stopped) = discovery.take();
                if let Some(event) = event.filter(|event| event.generation == discovery.generation()) {
                    self.pending_catalog = Some(CatalogDelivery::new(event));
                }
                if stopped {
                    self.discovery = None;
                    self.discovery_retry_at = Some(Instant::now() + Duration::from_secs(1));
                    // Latest metadata may coalesce. A final success from a dead
                    // producer must not clear the owning failure/retry warning.
                    if self
                        .pending_catalog
                        .as_ref()
                        .is_none_or(|delivery| delivery.event.result.is_ok())
                    {
                        self.pending_catalog = Some(CatalogDelivery::new(osc_discovery::DiscoveryEvent {
                            generation: 0,
                            result: Err("OSC interface discovery stopped; retrying".into()),
                        }));
                    }
                }
            }
        }
        if let Some(delivery) = self.pending_catalog.as_mut() {
            let snapshot = ctx.tree_snapshot_arc();
            let target = self.network_interface.is_bound().then(|| self.network_interface.id());
            if delivery.service(ctx, snapshot.as_deref(), target) {
                self.pending_catalog = None;
            }
        }
    }

    pub(super) fn service(&mut self, ctx: &mut ProcessCtx) {
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
        if !self.node_data().effective_enabled {
            return;
        }
        self.service_discovery(ctx);
        self.drain_transport_events(ctx);
        if let Some(delivery) = self.incoming.as_mut() {
            let snapshot = ctx.tree_snapshot_arc();
            if delivery.service(
                ctx,
                snapshot.as_deref(),
                self.base.values_id(),
                self.auto_add.get(),
                &mut self.ignored_param_changes,
            ) {
                self.incoming = None;
                self.ignored_param_changes.clear();
            }
        }
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.refresh_data_capabilities(ctx, &snapshot);
            if self.transport_dirty {
                self.refresh_transport(ctx, &snapshot);
            }
            self.flush_outbound_values(ctx, &snapshot);
        }
        self.arm_service();
    }

    fn arm_service(&self) {
        let now = Instant::now();
        let transport = self.transport_dirty && self.retiring_transport.is_none();
        let outgoing = !self.pending_outbound_nodes.is_empty();
        let immediate = self.incoming.is_some()
            || self.pending_catalog.is_some()
            || self.discovery.as_ref().is_some_and(Discovery::has_pending)
            || self.transport.as_ref().is_some_and(OscTransportHandle::has_pending)
            || self
                .retiring_transport
                .as_ref()
                .is_some_and(OscTransportHandle::has_pending)
            || transport && self.retry_at.is_none_or(|at| at <= now)
            || outgoing && self.outbound_retry_at.is_none_or(|at| at <= now);
        if immediate {
            self.request_service();
        }
        let next = [
            transport.then_some(self.retry_at).flatten(),
            outgoing.then_some(self.outbound_retry_at).flatten(),
            self.discovery.is_none().then_some(self.discovery_retry_at).flatten(),
        ]
        .into_iter()
        .flatten()
        .filter(|at| *at > now)
        .min();
        if let (Some(wake), Some(next)) = (&self.wake, next) {
            wake.later(next.saturating_duration_since(now));
        }
    }
}
