//! Retry window admission only after every immediately ready output has been delivered.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    Poll,
    Retry { timeout_ms: u32 },
}

impl Admission {
    pub(crate) fn timeout_ms(self) -> u32 {
        match self {
            Self::Poll => 0,
            Self::Retry { timeout_ms } => timeout_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Attempt {
    Complete,
    DeferredWindow,
}

pub(crate) struct OutputSchedule {
    deferred: Vec<usize>,
}

impl OutputSchedule {
    pub(crate) fn new(max_outputs: usize) -> Self {
        Self {
            deferred: Vec::with_capacity(max_outputs),
        }
    }

    pub(crate) fn submit(&mut self, outputs: usize, mut attempt: impl FnMut(usize, Admission) -> Attempt) {
        debug_assert!(outputs <= self.deferred.capacity(), "validated native output bindings");
        self.deferred.clear();
        for index in 0..outputs {
            if attempt(index, Admission::Poll) == Attempt::DeferredWindow {
                self.deferred.push(index);
            }
        }
        for (position, index) in self.deferred.iter().copied().enumerate() {
            // At most one positive kernel timeout for the entire submission, on this GPU
            // worker. Other ready outputs have already delivered; later retries only poll.
            // Preserve plan order as the priority among initially busy windows.
            let result = attempt(
                index,
                Admission::Retry {
                    timeout_ms: u32::from(position == 0),
                },
            );
            debug_assert_eq!(result, Attempt::Complete, "a retry must record its final outcome");
        }
    }
}

#[cfg(test)]
mod tests;
