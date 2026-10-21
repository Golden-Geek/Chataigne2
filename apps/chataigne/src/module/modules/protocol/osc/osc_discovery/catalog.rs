use golden_core::parameter::{ParamValue, ParameterEnumOption, ParameterEnumOptions};
use golden_io::PayloadPool;
use std::{mem::size_of, net::Ipv4Addr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Selection {
    Any,
    Address(Ipv4Addr),
}
impl Selection {
    pub(in super::super) fn parse(value: &str) -> Result<Self, String> {
        if value.len() > 64 {
            return Err("OSC interface selection exceeds 64 bytes".into());
        }
        if value == "any" || value.trim().is_empty() {
            return Ok(Self::Any);
        }
        value
            .parse()
            .map(Self::Address)
            .map_err(|_| "OSC interface must be Any or an IPv4 address".into())
    }
    pub(in super::super) fn variant(self) -> String {
        match self {
            Self::Any => "any".into(),
            Self::Address(address) => address.to_string(),
        }
    }
    pub(in super::super) fn bind_host(self) -> String {
        match self {
            Self::Any => "0.0.0.0".into(),
            Self::Address(address) => address.to_string(),
        }
    }
}
impl std::fmt::Display for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Any => f.write_str("any"),
            Self::Address(address) => address.fmt(f),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in super::super) struct Catalog {
    pub options: ParameterEnumOptions,
    pub selection: Selection,
    pub available: bool,
}
impl Catalog {
    pub(super) fn prepare(
        mut options: Vec<ParameterEnumOption>,
        selection: Selection,
        previous: Option<&Self>,
        pool: &PayloadPool,
    ) -> Result<Self, String> {
        if options.capacity() > 64 {
            return Err("OSC interface catalog exceeds 64 retained options".into());
        }
        if !options.iter().any(|option| option.variant_id == "any") {
            return Err("OSC interface catalog is missing the Any option".into());
        }
        let mut variants = std::collections::BTreeSet::new();
        for option in &options {
            if !variants.insert(option.variant_id.as_str()) {
                return Err("OSC interface catalog contains duplicate variants".into());
            }
            if option.label.capacity() > 1024
                || option.variant_id.capacity() > 64
                || option.tags.capacity() > 8
                || option.tags.iter().any(|tag| tag.capacity() > 64)
                || !matches!(&option.value, ParamValue::Enum(value) if value == &option.variant_id && value.capacity() <= 64)
                || Selection::parse(&option.variant_id).is_err()
            {
                return Err("OSC interface metadata exceeds its supported field bounds".into());
            }
        }
        let selected = selection.variant();
        let available = selection == Selection::Any || options.iter().any(|option| option.variant_id == selected);
        if !available {
            if options.len() == 64 {
                return Err("OSC interface catalog cannot retain the missing selection within 64 options".into());
            }
            options.reserve_exact(1);
            options.push(ParameterEnumOption {
                variant_id: selected.clone(),
                value: ParamValue::Enum(selected.clone()),
                label: format!("Unavailable ({selected})"),
                tags: vec!["unavailable".into()],
                ordering: Some(20),
            });
        }
        // Vec growth on the final missing-selection row is included in admission.
        if options.capacity() > 64 {
            return Err("OSC interface catalog exceeds 64 retained slots".into());
        }
        options.sort_by(|a, b| (a.ordering, &a.label).cmp(&(b.ordering, &b.label)));
        let mut domain: ParameterEnumOptions = options.into();
        if domain.retained_bytes() > 64 * 1024 {
            return Err("OSC interface catalog exceeds 64 KiB".into());
        }
        if let Some(previous) = previous.filter(|previous| previous.options == domain) {
            domain = previous.options.clone();
        } else {
            pool.try_retain_arc(domain.shared().clone(), domain.retained_bytes())
                .map_err(|_| "OSC interface catalog lifetime capacity exhausted".to_string())?;
        }
        Ok(Self {
            options: domain,
            selection,
            available,
        })
    }
    pub(super) fn weight(&self) -> usize {
        size_of::<Self>() + self.options.retained_bytes()
    }
}
