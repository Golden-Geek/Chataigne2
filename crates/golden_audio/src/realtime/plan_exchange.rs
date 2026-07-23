use std::fmt;

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use super::{QueuePressureCounters, assert_not_realtime};

#[derive(Debug)]
pub enum PlanPublishError<T> {
    Pending(Box<T>),
    QueueFull(Box<T>),
}

impl<T> PlanPublishError<T> {
    #[must_use]
    pub fn into_plan(self) -> Box<T> {
        match self {
            Self::Pending(plan) | Self::QueueFull(plan) => plan,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RealtimePlanSlotMetrics {
    pub swaps: u64,
    pub retained_returns: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanSwapResult {
    Unchanged,
    ReturnedRetainedPlan,
    Swapped,
    SwappedAndRetainedOldPlan,
}

pub struct RenderPlanPublisher<T> {
    pending_producer: Producer<Box<T>>,
    retired_consumer: Consumer<ReturnedPlan<T>>,
    pending: bool,
    pressure: QueuePressureCounters,
}

impl<T> fmt::Debug for RenderPlanPublisher<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RenderPlanPublisher")
            .field("pending", &self.pending)
            .field("pressure", &self.pressure.snapshot())
            .finish_non_exhaustive()
    }
}

impl<T> RenderPlanPublisher<T> {
    pub fn publish(&mut self, plan: Box<T>) -> Result<(), PlanPublishError<T>> {
        assert_not_realtime("render-plan publication");
        self.reclaim_acknowledged();
        if self.pending {
            return Err(PlanPublishError::Pending(plan));
        }
        match self.pending_producer.push(plan) {
            Ok(()) => {
                self.pending = true;
                Ok(())
            }
            Err(PushError::Full(plan)) => {
                self.pressure.plan_publish_full();
                Err(PlanPublishError::QueueFull(plan))
            }
        }
    }

    /// Reclaims acknowledged retired plans on the control thread.
    pub fn reclaim_acknowledged(&mut self) -> usize {
        assert_not_realtime("render-plan reclamation");
        let mut reclaimed = 0;
        while let Ok(returned) = self.retired_consumer.pop() {
            drop(returned.plan);
            if returned.acknowledges_pending {
                self.pending = false;
            }
            reclaimed += 1;
        }
        reclaimed
    }

    #[must_use]
    pub const fn has_pending_plan(&self) -> bool {
        self.pending
    }

    #[must_use]
    pub fn pressure(&self) -> QueuePressureSnapshot {
        self.pressure.snapshot()
    }
}

use super::QueuePressureSnapshot;

pub struct RealtimePlanSlot<T> {
    active: Box<T>,
    pending_consumer: Consumer<Box<T>>,
    retired_producer: Producer<ReturnedPlan<T>>,
    retained_retired: Option<Box<T>>,
    metrics: RealtimePlanSlotMetrics,
    pressure: QueuePressureCounters,
}

impl<T> fmt::Debug for RealtimePlanSlot<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RealtimePlanSlot")
            .field("has_retained_retired", &self.retained_retired.is_some())
            .field("metrics", &self.metrics)
            .finish_non_exhaustive()
    }
}

impl<T> RealtimePlanSlot<T> {
    /// Applies at most one ownership transition at a block boundary.
    #[inline]
    pub fn begin_block(&mut self) -> PlanSwapResult {
        if let Some(retired) = self.retained_retired.take() {
            return match self.retired_producer.push(ReturnedPlan {
                plan: retired,
                acknowledges_pending: true,
            }) {
                Ok(()) => PlanSwapResult::ReturnedRetainedPlan,
                Err(PushError::Full(returned)) => {
                    self.retained_retired = Some(returned.plan);
                    self.pressure.plan_return_full();
                    self.metrics.retained_returns = self.metrics.retained_returns.saturating_add(1);
                    PlanSwapResult::Unchanged
                }
            };
        }

        let Ok(next) = self.pending_consumer.pop() else {
            return PlanSwapResult::Unchanged;
        };
        let retired = std::mem::replace(&mut self.active, next);
        self.metrics.swaps = self.metrics.swaps.saturating_add(1);
        match self.retired_producer.push(ReturnedPlan {
            plan: retired,
            acknowledges_pending: true,
        }) {
            Ok(()) => PlanSwapResult::Swapped,
            Err(PushError::Full(returned)) => {
                self.retained_retired = Some(returned.plan);
                self.pressure.plan_return_full();
                self.metrics.retained_returns = self.metrics.retained_returns.saturating_add(1);
                PlanSwapResult::SwappedAndRetainedOldPlan
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn active(&self) -> &T {
        self.active.as_ref()
    }

    #[inline]
    #[must_use]
    pub const fn metrics(&self) -> RealtimePlanSlotMetrics {
        self.metrics
    }

    #[must_use]
    pub fn pressure(&self) -> QueuePressureSnapshot {
        self.pressure.snapshot()
    }

    /// Transfers all callback-owned plan references to a value that must be destroyed off callback.
    #[must_use]
    pub fn retire(self) -> RealtimePlanRetirement<T> {
        RealtimePlanRetirement {
            active: self.active,
            retained_retired: self.retained_retired,
        }
    }

    #[cfg(test)]
    pub(super) fn fill_return_queue_for_test(&mut self, plan: Box<T>) {
        assert!(
            self.retired_producer
                .push(ReturnedPlan {
                    plan,
                    acknowledges_pending: false,
                })
                .is_ok(),
            "test return queue has space"
        );
    }
}

struct ReturnedPlan<T> {
    plan: Box<T>,
    acknowledges_pending: bool,
}

pub struct RealtimePlanRetirement<T> {
    active: Box<T>,
    retained_retired: Option<Box<T>>,
}

impl<T> fmt::Debug for RealtimePlanRetirement<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RealtimePlanRetirement")
            .field("has_active", &true)
            .field("has_retained_retired", &self.retained_retired.is_some())
            .finish()
    }
}

impl<T> RealtimePlanRetirement<T> {
    pub fn reclaim(self) {
        assert_not_realtime("final render-plan reclamation");
        let Self {
            active,
            retained_retired,
        } = self;
        drop(active);
        drop(retained_retired);
    }
}

#[must_use]
pub fn acknowledged_plan_exchange<T>(active: Box<T>) -> (RenderPlanPublisher<T>, RealtimePlanSlot<T>) {
    let pressure = QueuePressureCounters::default();
    let (pending_producer, pending_consumer) = RingBuffer::new(1);
    let (retired_producer, retired_consumer) = RingBuffer::new(1);
    (
        RenderPlanPublisher {
            pending_producer,
            retired_consumer,
            pending: false,
            pressure: pressure.clone(),
        },
        RealtimePlanSlot {
            active,
            pending_consumer,
            retired_producer,
            retained_retired: None,
            metrics: RealtimePlanSlotMetrics::default(),
            pressure,
        },
    )
}
