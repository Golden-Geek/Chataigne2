use super::OscModuleBase;
impl OscModuleBase {
    pub(crate) fn set_transport_dirty(&mut self, dirty: bool) {
        self.transport_dirty = dirty;
    }
    pub(crate) fn transport_settled_for_test(&self) -> bool {
        !self.transport_dirty
            && self.retiring_transport.is_none()
            && self.retry_at.is_none()
            && self
                .transport
                .as_ref()
                .is_none_or(|transport| self.transport_ready && transport.is_ready())
    }
    pub(crate) fn transport_retrying_for_test(&self) -> bool {
        self.retry_at.is_some()
    }
    pub(crate) fn transport_state_for_test(&self) -> String {
        format!(
            "active={}, retiring={}, dirty={}, retry={:?}, ready={}, config={:?}",
            self.transport.is_some(),
            self.retiring_transport.is_some(),
            self.transport_dirty,
            self.retry_at,
            self.transport_ready,
            self.last_transport_config
        )
    }
}
