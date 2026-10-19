use crate::{
    ScriptInputError,
    input::{self, MAX_INPUT_BYTES, MAX_LABEL_BYTES},
};
use golden_io::{PayloadReservation, RetainedPayload};
use golden_parameters::ParamValue;

/// An admitted property intent whose original value is disposed on the input worker.
#[derive(Clone, Debug)]
pub struct ScriptPropertyWrite {
    property: String,
    value: RetainedPayload<ParamValue>,
}

/// Storage reserved before materializing a typed property value.
pub struct ScriptPropertyReservation {
    property: String,
    permit: PayloadReservation,
}

impl ScriptPropertyWrite {
    /// Prepare at the producer boundary before queueing an edit. Failed admission
    /// returns the original key/value allocations to that producer.
    pub fn prepare(
        property: impl Into<String>,
        value: ParamValue,
    ) -> Result<Self, ScriptInputError<(String, ParamValue)>> {
        let property = property.into();
        let admission = || {
            if property.capacity() > MAX_LABEL_BYTES {
                return Err("script property exceeds 1024 retained bytes");
            }
            let weight = input::param_weight(&value)?.saturating_add(64);
            if weight > MAX_INPUT_BYTES {
                return Err("script property exceeds 4 MiB retained storage");
            }
            Ok((input::reserve(weight)?, weight))
        };
        match admission() {
            Ok((permit, weight)) => Ok(Self {
                property,
                value: permit.retain(value, weight).expect("property fits admission"),
            }),
            Err(message) => Err(ScriptInputError::new((property, value), message)),
        }
    }

    /// Reserve before converting untrusted JSON into a property value.
    pub fn reserve(property: &str) -> Result<ScriptPropertyReservation, String> {
        if property.len() > MAX_LABEL_BYTES {
            return Err("script property exceeds 1024 bytes".into());
        }
        Ok(ScriptPropertyReservation {
            property: property.to_owned(),
            permit: input::reserve(MAX_INPUT_BYTES)?,
        })
    }

    /// Bounded property name.
    pub fn property(&self) -> &str {
        &self.property
    }
    /// Original immutable value, without cloning its storage.
    pub fn value(&self) -> &ParamValue {
        &self.value
    }
}

impl ScriptPropertyReservation {
    /// Retain the converted value and release unused reserved bytes.
    pub fn finish(self, value: ParamValue) -> Result<ScriptPropertyWrite, ScriptInputError<(String, ParamValue)>> {
        let weight = input::param_weight(&value).map(|bytes| bytes.saturating_add(64));
        match weight {
            Ok(weight) if weight <= MAX_INPUT_BYTES => Ok(ScriptPropertyWrite {
                property: self.property,
                value: self.permit.retain(value, weight).expect("property fits admission"),
            }),
            _ => Err(ScriptInputError::new(
                (self.property, value),
                "script property exceeds its retained-storage limit",
            )),
        }
    }
}

#[cfg(test)]
mod tests;
