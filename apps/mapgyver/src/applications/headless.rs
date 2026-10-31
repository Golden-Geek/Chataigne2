//! App policy on a headless host owner; actor access only captures changed metadata.
use super::{driver::ApplicationDriver, protocol::ApplicationEdit, runtime::ApplicationCallbackSubscription};
use super::{
    protocol::ApplicationHost,
    runtime::supervision::{Source, Supervisor},
};
use crate::{
    app::AppNode,
    media::protocol::{MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit},
};
use golden_core::application::{ProductionRuntime, ProjectTransactions};
use golden_io::{BoundedWorker, RetirementPool, WorkerLimits};
use std::{
    io,
    num::NonZeroUsize,
    sync::OnceLock,
    time::{Duration, Instant},
};

/// Cancels supervision without joining process cleanup on the caller's thread.
pub struct HeadlessApplications {
    _worker: BoundedWorker<(), ()>,
}
impl HeadlessApplications {
    /// Starts only on the authoritative headless runtime, never candidate preparation or Undo.
    pub fn start(runtime: ProductionRuntime<AppNode>) -> io::Result<Self> {
        static POOL: OnceLock<RetirementPool> = OnceLock::new();
        let pool = POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(2).unwrap()));
        let worker = BoundedWorker::spawn(
            "mapgyver-headless-applications",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |context| {
                let notifier = context.wake_notifier();
                let mut driver = ApplicationDriver::default();
                let mut callbacks: Option<(uuid::Uuid, ApplicationCallbackSubscription)> = None;
                let mut retry = Instant::now();
                let mut callback_retry = retry;
                let mut failure = None;
                let mut supervisor: Option<Supervisor> = None;
                let mut supervision_retry = retry;
                while !context.cancellation().is_cancelled() {
                    let checkpoint = context.wake_checkpoint();
                    if supervisor.as_ref().is_some_and(|owner| !owner.is_current()) {
                        supervisor.take();
                    }
                    if supervisor.is_none() && Instant::now() >= supervision_retry {
                        match runtime
                            .capture_app(Source::capture)
                            .map_err(|error| error.to_string())
                            .and_then(|source| source)
                            .and_then(|source| source.claim(ApplicationHost::Headless))
                        {
                            Ok(owner) => supervisor = Some(owner),
                            Err(error) => {
                                eprintln!("[mapgyver-headless-applications] supervision: {error}");
                                supervision_retry = Instant::now() + Duration::from_secs(1);
                            }
                        }
                    }
                    let Some(supervisor) = &supervisor else {
                        context.wait_until(checkpoint, Instant::now() + Duration::from_millis(100));
                        continue;
                    };
                    if driver.needs_service() && Instant::now() >= retry {
                        let result = runtime.capture_app(driver.service_capture());
                        let result = match result {
                            Ok(capture) => capture.and_then(|capture| {
                                supervisor.validate_project(capture.project())?;
                                driver.apply_prepared(capture, notifier.clone())
                            }),
                            Err(error) => Err(error.to_string()),
                        };
                        supervisor
                            .publisher()
                            .demand(result.as_ref().map(|_| ()).map_err(Clone::clone));
                        match result {
                            Ok(_) => failure = None,
                            Err(error) => {
                                if failure.as_ref() != Some(&error) {
                                    eprintln!("[mapgyver-headless-applications] {error}");
                                }
                                failure = Some(error);
                                retry = Instant::now() + Duration::from_secs(1);
                            }
                        }
                    }
                    let source = Some(supervisor.callbacks()).filter(|source| source.is_current());
                    if source.as_ref().map(|source| source.project()) != callbacks.as_ref().map(|(project, _)| *project)
                    {
                        callbacks.take();
                    }
                    if let Some(source) = source
                        && callbacks.is_none()
                        && Instant::now() >= callback_retry
                    {
                        let runtime = runtime.clone();
                        let publisher = supervisor.publisher();
                        let attempt = publisher.callback_attempt();
                        match source.start(move |ticket| {
                            let result = runtime
                                .apply_transaction(golden_core::ui_sync::UiEditIntent::InvokeAppOperation {
                                    namespace: MEDIA_NAMESPACE.into(),
                                    operation: MEDIA_OPERATION.into(),
                                    payload: serde_json::to_value(MediaEdit::Applications {
                                        edit: ApplicationEdit::DispatchCallbacks { ticket },
                                    })
                                    .map_err(|error| error.to_string())?,
                                })
                                .map(|_| ())
                                .map_err(|error| error.to_string());
                            publisher.callbacks(result.clone());
                            result
                        }) {
                            Ok(subscription) => {
                                callbacks = Some((source.project(), subscription));
                                supervisor.publisher().callbacks_started(attempt);
                            }
                            Err(error) => {
                                supervisor.publisher().callbacks(Err(error.clone()));
                                eprintln!("[mapgyver-headless-applications] callback admission: {error}");
                                callback_retry = Instant::now() + Duration::from_secs(1);
                            }
                        }
                    }
                    // Guard checks are off-actor; unchanged projects perform no engine inspection.
                    context.wait_until(checkpoint, Instant::now() + Duration::from_millis(100));
                }
            },
        )
        .map_err(io::Error::other)?;
        Ok(Self { _worker: worker })
    }
}
