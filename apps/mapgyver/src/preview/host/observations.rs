use crate::preview::protocol::PreviewStatus;

#[derive(Default)]
pub(super) struct Observations {
    generation: Option<u64>,
    current: Vec<PreviewStatus>,
}

impl Observations {
    /// Idle/retired owners publish an empty set through the same delivery path as live frames.
    /// Failed delivery must remain retryable, including an unchanged reconnect snapshot.
    pub(super) fn publish(
        &mut self,
        generation: u64,
        statuses: Vec<PreviewStatus>,
        emit: impl FnOnce(&[PreviewStatus]) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.generation == Some(generation) && self.current == statuses {
            return Ok(());
        }
        emit(&statuses)?;
        self.current = statuses;
        self.generation = Some(generation);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
