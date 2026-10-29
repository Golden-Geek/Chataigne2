/// One endpoint allocation. A failure withdraws use immediately, but its object remains charged
/// until the completion covering its last attempted native call has actually finished.
pub(super) struct Endpoint<T> {
    value: Option<T>,
    last_use: u64,
    retire_after: Option<u64>,
}

impl<T> Endpoint<T> {
    pub fn new() -> Self {
        Self {
            value: None,
            last_use: 0,
            retire_after: None,
        }
    }
    pub fn create<E>(&mut self, factory: impl FnOnce() -> Result<T, E>) -> Result<bool, E> {
        if self.value.is_some() || self.retire_after.is_some() {
            return Ok(false);
        }
        self.value = Some(factory()?);
        self.last_use = 0;
        Ok(true)
    }
    pub fn can_create(&self) -> bool {
        self.value.is_none() && self.retire_after.is_none()
    }
    pub fn begin_use(&mut self, serial: u64) -> Option<&mut T> {
        if self.retire_after.is_some() {
            return None;
        }
        let value = self.value.as_mut()?;
        self.last_use = serial;
        Some(value)
    }
    pub fn fail(&mut self) {
        if self.value.is_some() {
            self.retire_after = Some(self.last_use);
        }
    }
    pub fn retire(&mut self, completed: u64) -> bool {
        if self.retire_after.is_some_and(|serial| serial <= completed) {
            // Drop finishes before another factory can acquire this same fixed storage slot.
            drop(self.value.take());
            self.retire_after = None;
            self.last_use = 0;
            true
        } else {
            false
        }
    }
    pub fn clear(&mut self) {
        drop(self.value.take());
        self.retire_after = None;
    }
}
