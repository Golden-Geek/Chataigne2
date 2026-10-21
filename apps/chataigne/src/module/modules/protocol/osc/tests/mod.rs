use super::OscModuleBase;
impl OscModuleBase {
    pub(crate) fn fake_discovery_for_test(&mut self) {
        use golden_core::parameter::{ParamValue, ParameterEnumOption};
        self.pending_catalog = None;
        self.discovery = Some(
            super::osc_discovery::Discovery::spawn_fake(
                super::osc_discovery::Selection::parse(self.network_interface.get_ref().as_str()).unwrap(),
                self.wake.as_ref().unwrap().notifier.clone(),
                || {
                    Ok(vec![ParameterEnumOption {
                        variant_id: "any".into(),
                        value: ParamValue::Enum("any".into()),
                        label: "Any".into(),
                        tags: Vec::new(),
                        ordering: Some(0),
                    }])
                },
            )
            .unwrap(),
        );
        self.request_service();
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
