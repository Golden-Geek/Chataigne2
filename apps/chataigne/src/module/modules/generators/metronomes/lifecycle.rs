use super::{metronome_values_tree, runtime, runtime_update_rate_hz, sync_value_folders, MetronomesModule};
use crate::app::module::common::generators::AdmissionError;
use golden_core::{
    node::{Node, NodeHandle},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use std::time::{Duration, Instant};

impl MetronomesModule {
    pub(super) fn sync_configuration(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if !snapshot.node(self.id()).is_some_and(|node| node.enabled) {
            self.stop_runtime();
            self.config_dirty = false;
            return;
        }
        if self.wake.is_none() {
            self.config_dirty = true;
            return;
        }
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            self.config_dirty = true;
            self.request_service();
            return;
        }
        self.retry_at = None;
        let configs = match self.collect_metronomes(snapshot) {
            Ok(configs) => configs,
            Err(error) => {
                self.stop_runtime();
                self.runtime_warning(ctx, &error);
                self.base.set_connected(ctx, false);
                self.config_dirty = false;
                return;
            }
        };
        let Some(values_root) = self.values_root(snapshot) else {
            self.config_dirty = false;
            self.pending_value_items.clear();
            self.stop_runtime();
            return;
        };
        let waiting_for_values = sync_value_folders(
            ctx,
            snapshot,
            values_root,
            configs.iter(),
            &mut self.value_nodes_by_item,
            &mut self.pending_value_items,
            metronome_values_tree,
        );
        self.config_dirty = waiting_for_values;

        self.configure_runtime(
            ctx,
            runtime::MetronomeWorkerConfig {
                update_rate_hz: runtime_update_rate_hz(self.update_rate_hz.get()),
                metronomes: configs,
            },
        );
    }

    fn configure_runtime(&mut self, ctx: &mut ProcessCtx, config: runtime::MetronomeWorkerConfig) {
        if let Some(runtime) = &self.runtime {
            match runtime.configure(config) {
                Ok(()) => self.runtime_ready(ctx),
                Err(AdmissionError::Full) => {
                    self.runtime_warning(ctx, "Generator busy; configuration will retry.");
                    self.retry_configuration();
                }
                Err(AdmissionError::Closed) => {
                    self.runtime_warning(ctx, "Generator worker stopped; retrying.");
                    self.base.set_connected(ctx, false);
                    self.stop_runtime();
                    self.retry_configuration();
                }
                Err(error) => {
                    self.stop_runtime();
                    self.runtime_warning(ctx, &error.to_string());
                    self.base.set_connected(ctx, false);
                }
            }
        } else if let Some(wake) = &self.wake {
            match runtime::MetronomeRuntimeHandle::spawn(config, wake.notifier.clone()) {
                Ok(runtime) => {
                    self.runtime = Some(runtime);
                    self.runtime_ready(ctx);
                }
                Err(error) => {
                    self.runtime_warning(ctx, &error);
                    self.base.set_connected(ctx, false);
                    self.retry_configuration();
                }
            }
        }
    }

    pub(super) fn runtime_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some("generator_runtime"), message, None);
    }

    fn runtime_ready(&mut self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some("generator_runtime"));
        self.base.set_connected(ctx, true);
    }

    pub(super) fn retry_configuration(&mut self) {
        self.config_dirty = true;
        self.retry_at = Some(Instant::now() + Duration::from_millis(250));
        self.request_service();
    }

    pub(super) fn request_service(&self) {
        let Some(wake) = &self.wake else { return };
        if self.runtime.as_ref().is_some_and(|runtime| runtime.has_pending())
            || self.config_dirty && self.retry_at.is_none()
        {
            wake.notifier.notify();
        } else if self.config_dirty {
            wake.later(self.retry_at.map_or(Duration::from_millis(1), |at| {
                at.saturating_duration_since(Instant::now())
            }));
        }
    }

    pub(super) fn stop_runtime(&mut self) {
        if let Some(mut runtime) = self.runtime.take() {
            runtime.stop();
        }
        self.retry_at = None;
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
    }
}
