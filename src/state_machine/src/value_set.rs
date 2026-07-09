use std::sync::Arc;

use golden_alchemist::{ContextKey, ExtensionValue, RuntimeValue, StableRef, ValueTypeId};

pub const VALUE_SET_TYPE: &str = "chataigne.value_set";

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ValueLaneKey(String);

impl ValueLaneKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueSetError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ValueSetError::EmptyLaneKey);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[must_use]
pub fn lane_scoped_stable_ref(reference: &StableRef, context_key: &ContextKey) -> StableRef {
    let lane_id = context_key
        .iter()
        .map(|part| format!("{}={}", part.axis.as_str(), part.item.as_str()))
        .collect::<Vec<_>>()
        .join(";");
    StableRef::new(
        reference.value_type.clone(),
        format!("{}::lane::{lane_id}", reference.stable_id),
    )
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueSetEntry {
    pub key: ValueLaneKey,
    pub label: String,
    pub source: Option<StableRef>,
    pub value: RuntimeValue,
}

impl ValueSetEntry {
    #[must_use]
    pub fn new(key: ValueLaneKey, label: impl Into<String>, value: RuntimeValue) -> Self {
        Self {
            key,
            label: label.into(),
            source: None,
            value,
        }
    }

    #[must_use]
    pub fn with_source(mut self, source: StableRef) -> Self {
        self.source = Some(source);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueSet {
    pub entries: Vec<ValueSetEntry>,
    pub logical_tick: u64,
}

impl ValueSet {
    #[must_use]
    pub fn new(logical_tick: u64) -> Self {
        Self {
            entries: Vec::new(),
            logical_tick,
        }
    }

    #[must_use]
    pub fn with_entries(logical_tick: u64, entries: Vec<ValueSetEntry>) -> Self {
        Self { entries, logical_tick }
    }

    pub fn push(&mut self, entry: ValueSetEntry) {
        self.entries.push(entry);
    }

    pub fn to_runtime_value(&self) -> Result<RuntimeValue, ValueSetError> {
        let payload = serde_json::to_vec(self).map_err(ValueSetError::Encode)?;
        Ok(RuntimeValue::Extension(ExtensionValue::new(
            ValueTypeId::new(VALUE_SET_TYPE),
            Arc::<[u8]>::from(payload),
        )))
    }

    pub fn from_runtime_value(value: &RuntimeValue) -> Result<Self, ValueSetError> {
        match value {
            RuntimeValue::Extension(extension) if extension.value_type.as_str() == VALUE_SET_TYPE => {
                serde_json::from_slice(&extension.payload).map_err(ValueSetError::Decode)
            }
            RuntimeValue::Extension(extension) => Err(ValueSetError::WrongValueType {
                expected: VALUE_SET_TYPE,
                actual: extension.value_type.to_string(),
            }),
            value => Err(ValueSetError::WrongValueType {
                expected: VALUE_SET_TYPE,
                actual: value.value_type().to_string(),
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValueSetError {
    #[error("ValueSet lane keys must not be empty")]
    EmptyLaneKey,
    #[error("failed to encode ValueSet payload: {0}")]
    Encode(serde_json::Error),
    #[error("failed to decode ValueSet payload: {0}")]
    Decode(serde_json::Error),
    #[error("expected `{expected}` runtime value, got `{actual}`")]
    WrongValueType { expected: &'static str, actual: String },
}
