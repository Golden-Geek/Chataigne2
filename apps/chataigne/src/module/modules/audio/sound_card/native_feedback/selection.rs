//! Bounded selector capture on the actor; parsing and matching on the native owner.
use super::*;
use golden_io::{PayloadPool, RetainedPayload};

const SELECTOR_BYTES: usize = 256 * 1024;
const INPUT_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct Controls {
    parameter: NodeId,
    pub(super) driver: String,
    pub(super) value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum SelectionProgress {
    Waiting,
    Ready(bool),
    Failed,
}

#[derive(Default)]
pub(in super::super) struct SelectionDelivery {
    current: Option<Attempt>,
    retry_at: Option<Instant>,
    #[cfg(test)]
    pub(in super::super) checks: usize,
}

struct Attempt {
    inputs: RetainedPayload<Controls>,
    completion: Option<Arc<Completion>>,
    outcome: Option<Result<bool, ()>>,
}

impl SelectionDelivery {
    pub(in super::super) fn service(
        &mut self,
        event: &PreparedAudioEvent,
        module: &super::super::SoundCardModule,
    ) -> SelectionProgress {
        let golden_audio::AudioEvent::DeviceStatusChanged(status) = &event.event else {
            return SelectionProgress::Ready(false);
        };
        let Some(parameter) = module.device_parameter(status.direction) else {
            self.current = None;
            return SelectionProgress::Ready(false);
        };
        let (input, output) = module.selected_device_values();
        let value = if status.direction == golden_audio::AudioDirection::Input {
            input
        } else {
            output
        };
        self.service_using(
            &SERVICE,
            event,
            parameter,
            &module.audio_driver.get_ref().0,
            value,
            module.runtime_wake.as_ref(),
        )
    }

    fn service_using(
        &mut self,
        service: &Service,
        event: &PreparedAudioEvent,
        parameter: NodeId,
        driver: &str,
        value: &str,
        wake: Option<&RuntimeWakeSender>,
    ) -> SelectionProgress {
        if driver == super::super::NO_AUDIO_DRIVER || value == super::super::NO_AUDIO_DEVICE {
            self.current = None;
            self.retry_at = None;
            return SelectionProgress::Ready(false);
        }
        // Inspect borrowed lengths before cloning or parsing anything. Rejected controls
        // cannot enter either the result cache or a queued job.
        if driver.len() > super::super::DRIVER_SELECTION_BYTES || value.len() > SELECTOR_BYTES {
            self.current = None;
            self.retry_at = None;
            return SelectionProgress::Failed;
        }
        if self
            .current
            .as_ref()
            .is_some_and(|a| a.inputs.parameter != parameter || a.inputs.driver != driver || a.inputs.value != value)
        {
            // The old worker still owns its input/event and ticket until it finishes.
            // Its result has no side effects and cannot publish for this new selection.
            self.current = None;
        }
        if let Some(attempt) = &mut self.current {
            if let Some(completion) = &attempt.completion {
                let outcome = match completion.status.load(Ordering::Acquire) {
                    PENDING => return SelectionProgress::Waiting,
                    MATCHED => Ok(true),
                    UNMATCHED => Ok(false),
                    _ => Err(()),
                };
                attempt.completion = None;
                attempt.outcome = Some(outcome);
            }
            return match attempt.outcome.expect("settled selection") {
                Ok(matches) => SelectionProgress::Ready(matches),
                Err(()) => SelectionProgress::Failed,
            };
        }
        if let Some(at) = self.retry_at.filter(|at| Instant::now() < *at) {
            if let Some(wake) = wake {
                wake.wake_after(at.saturating_duration_since(Instant::now()));
            }
            return SelectionProgress::Waiting;
        }
        let admitted = service.capture(parameter, driver, value).and_then(|inputs| {
            service
                .submit_operation(event.clone(), Operation::Selection(inputs.clone()), wake.cloned())
                .map(|completion| Attempt {
                    inputs,
                    completion: Some(completion),
                    outcome: None,
                })
        });
        match admitted {
            Ok(attempt) => {
                self.current = Some(attempt);
                self.retry_at = None;
                #[cfg(test)]
                {
                    self.checks += 1;
                }
            }
            Err(()) => {
                self.retry_at = Some(Instant::now() + RETRY_DELAY);
                if let Some(wake) = wake {
                    wake.wake_after(RETRY_DELAY);
                }
            }
        }
        SelectionProgress::Waiting
    }
}

impl Service {
    fn capture(&self, parameter: NodeId, driver: &str, value: &str) -> Result<RetainedPayload<Controls>, ()> {
        let bytes = size_of::<Controls>() + 2 * size_of::<usize>() + driver.len() + value.len();
        let mut pool = self.inputs.lock().unwrap_or_else(|p| p.into_inner());
        if pool.is_none() {
            *pool = Some(PayloadPool::new("sound-card-feedback-controls", TICKETS, INPUT_BYTES).map_err(|_| ())?);
        }
        let reservation = pool.as_ref().unwrap().try_reserve(bytes).map_err(|_| ())?;
        // Reservation precedes both copies. Final source disposal is owned by the pool,
        // including cache invalidation while the native worker is still blocked.
        reservation
            .retain(
                Controls {
                    parameter,
                    driver: driver.to_owned(),
                    value: value.to_owned(),
                },
                bytes,
            )
            .map_err(|_| ())
    }
}

#[cfg(test)]
mod tests;
