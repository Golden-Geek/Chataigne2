use super::*;
use golden_io::{WorkerContext, WorkerReceive};
use std::time::{Duration, Instant};
struct Running {
    shared: Arc<Shared>,
    native: Option<RevisionSource>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.native = None;
        self.shared.running.store(false, Ordering::Release);
        self.shared.wake.notify();
    }
}
pub(super) fn run<P: Provider>(
    ctx: WorkerContext<Work, Event>,
    shared: Arc<Shared>,
    native: RevisionSource,
    reports: PayloadPool,
    commands: PayloadPool,
    open: impl FnOnce() -> Result<P, String>,
) {
    let running = Running {
        shared: shared.clone(),
        native: Some(native),
    };
    let mut provider = match open() {
        Ok(value) => value,
        Err(error) => {
            let _ = ctx.publish_weighted(Event::Error(diagnostic(error)), 2048);
            return;
        }
    };
    let _running = running;
    if ctx.cancellation().is_cancelled() {
        return;
    }
    let _ = ctx.publish_weighted(Event::Ready, 128);
    let mut next = Instant::now();
    let mut pending_report: Option<RetainedPayload<Report>> = None;
    let mut pending_command = None;
    let mut initial_pending = false;
    let mut last_config: Option<RetainedPayload<Config>> = None;
    while !ctx.cancellation().is_cancelled() {
        let mut batch = Vec::with_capacity(8);
        if let Some(command) = pending_command.take() {
            batch.push(command);
        }
        let remaining = 8 - batch.len();
        if remaining > 0 {
            ctx.drain_commands(&mut batch, NonZeroUsize::new(remaining).unwrap());
        }
        for command in batch {
            if ctx.cancellation().is_cancelled() {
                return;
            }
            command.run(&mut provider, &commands);
        }
        let config = shared
            .configuration
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let changed = config.as_ref().is_some_and(|value| {
            last_config
                .as_ref()
                .is_none_or(|previous| !std::ptr::eq::<Config>(&**previous, &*value.value))
        });
        if changed {
            initial_pending = true;
            next = Instant::now();
            last_config = config.as_ref().map(|value| value.value.clone());
        }
        if pending_report.as_ref().is_some_and(|report| !report.fence.current()) {
            pending_report = None;
        }
        if let Some(report) = pending_report.take() {
            match ctx.publish_weighted(Event::Report(report), REPORT_WEIGHT) {
                Ok(()) => {}
                Err(golden_io::PendingSendError::Full(Event::Report(report))) => pending_report = Some(report),
                Err(_) => return,
            }
        }
        if pending_report.is_none() && Instant::now() >= next {
            next = Instant::now() + Duration::from_millis(500);
            if let Some(config) = config.filter(|value| {
                value.fence.current()
                    && (initial_pending
                        || value
                            .value
                            .apps
                            .iter()
                            .any(|entry| !entry.target_path.trim().is_empty())
                        || value
                            .value
                            .folders
                            .iter()
                            .any(|entry| !entry.target_path.trim().is_empty()))
            }) {
                let result = (|| {
                    let reservation = reports
                        .try_reserve(REPORT_WEIGHT)
                        .map_err(|_| "App Control report capacity exhausted; sample withheld")?;
                    let rows = provider.sample(&config.value)?;
                    if rows.apps.len() != config.value.apps.len() || rows.folders.len() != config.value.folders.len() {
                        return Err("App Control provider returned an invalid report shape".into());
                    }
                    let mut callbacks = Vec::with_capacity(rows.folders.len());
                    for (index, row) in rows.folders.iter().enumerate() {
                        let callback = if row.update.has_changes() {
                            NativeCallbackPayload::prepare(
                                super::super::WATCH_FOLDER_CHANGED_CALLBACK,
                                vec![
                                    crate::app::module::script_api::node_arg(config.value.folders[index].item_id),
                                    serde_json::json!({
                                        "path":row.update.path,
                                        "exists":row.update.exists,
                                        "entryCount":row.update.entry_count,
                                        "created":row.update.created,
                                        "modified":row.update.modified,
                                        "removed":row.update.removed,
                                        "timestampMs":row.update.timestamp_ms,
                                    }),
                                ],
                            )
                            .map(Some)
                        } else {
                            Ok(None)
                        };
                        match callback {
                            Ok(value) => callbacks.push(value),
                            Err(error) => {
                                let _ = ctx.publish_weighted(Event::Warning(diagnostic(error)), 2048);
                                callbacks.push(None);
                            }
                        }
                    }
                    reservation
                        .retain(
                            Report {
                                config: config.value,
                                rows,
                                callbacks,
                                fence: config.fence,
                            },
                            REPORT_WEIGHT,
                        )
                        .map_err(|_| "App Control report reservation failed".into())
                })();
                match result {
                    Ok(report) => {
                        initial_pending = false;
                        pending_report = Some(report);
                    }
                    Err(error) => {
                        let _ = ctx.publish_weighted(Event::Warning(diagnostic(error)), 2048);
                    }
                }
            }
        }
        let deadline = if pending_report.is_some() {
            Instant::now() + Duration::from_millis(10)
        } else {
            next.min(Instant::now() + Duration::from_millis(50))
        };
        match ctx.receive_until(deadline) {
            WorkerReceive::Command(command) => pending_command = Some(command),
            WorkerReceive::Deadline => {}
            WorkerReceive::Disconnected => return,
        }
    }
}
