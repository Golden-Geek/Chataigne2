use crate::input::{
    self, MAX_INPUT_BYTES as MAX_ARGUMENT_BYTES, MAX_LABEL_BYTES as MAX_METHOD_BYTES, MAX_VALUES as MAX_ARGUMENTS,
};
use golden_io::{PayloadReservation, RetainedPayload};
use golden_parameters::ParamValue;
use std::{mem::size_of, ops::Deref};

fn argument_weight(args: &Vec<ParamValue>) -> Result<usize, &'static str> {
    if args.len() > MAX_ARGUMENTS {
        return Err("script method exceeds 4096 arguments");
    }
    let mut weight =
        (size_of::<Vec<ParamValue>>() + 64).saturating_add(args.capacity().saturating_mul(size_of::<ParamValue>()));
    for arg in args {
        if weight > MAX_ARGUMENT_BYTES {
            return Err("script method exceeds 4 MiB retained argument storage");
        }
        weight = weight.saturating_add(input::param_weight(arg)?.saturating_sub(size_of::<ParamValue>()));
    }
    if weight > MAX_ARGUMENT_BYTES {
        return Err("script method exceeds 4 MiB retained argument storage");
    }
    Ok(weight)
}

/// Immutable original arguments with bounded lifetime admission and background final disposal.
#[derive(Clone, Debug)]
pub struct ScriptArguments {
    values: RetainedPayload<Vec<ParamValue>>,
    weight: usize,
}

impl ScriptArguments {
    /// Retained storage including spare capacity and reference path/name allocations.
    pub fn retained_bytes(&self) -> usize {
        self.weight
    }

    /// Capacity of the original argument vector, without exposing an untracked owner.
    pub fn capacity(&self) -> usize {
        self.values.capacity()
    }
}

impl Deref for ScriptArguments {
    type Target = [ParamValue];
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

/// A prepared method intent. Queues and rejected edit suffixes own only shallow leases.
#[derive(Clone, Debug)]
pub struct ScriptMethodCall {
    method: String,
    args: ScriptArguments,
}

/// Producer-owned rejection. Large original data is returned rather than silently discarded.
pub struct ScriptCallPreparationError {
    method: String,
    args: Vec<ParamValue>,
    message: &'static str,
}

impl ScriptCallPreparationError {
    /// Recover the original producer allocations after admission failure.
    pub fn into_parts(self) -> (String, Vec<ParamValue>) {
        (self.method, self.args)
    }
}

impl std::fmt::Display for ScriptCallPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::fmt::Debug for ScriptCallPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ScriptCallPreparationError {}

/// Capacity admitted before QuickJS materializes typed arguments.
pub struct ScriptCallReservation {
    method: String,
    reservation: PayloadReservation,
}

impl ScriptMethodCall {
    /// Prepare on the producer boundary, before enqueueing an engine edit. Failure returns
    /// original allocations to that producer; this is not a consumer-side cleanup API.
    pub fn prepare(method: impl Into<String>, args: Vec<ParamValue>) -> Result<Self, ScriptCallPreparationError> {
        let method = method.into();
        let prepare = || {
            if method.capacity() > MAX_METHOD_BYTES {
                return Err("script method name exceeds 1024 retained bytes");
            }
            let weight = argument_weight(&args)?;
            let reservation = input::reserve(weight)?;
            Ok((reservation, weight))
        };
        match prepare() {
            Ok((reservation, weight)) => Ok(Self::seal(method, args, reservation, weight)),
            Err(message) => Err(ScriptCallPreparationError { method, args, message }),
        }
    }

    /// Reserve the maximum per-call storage before typed argument conversion. The parser
    /// must independently bound its scratch tree and retain partial conversion on failure.
    pub fn reserve(method: &str) -> Result<ScriptCallReservation, String> {
        if method.len() > MAX_METHOD_BYTES {
            return Err("script method name exceeds 1024 bytes".into());
        }
        let reservation = input::reserve(MAX_ARGUMENT_BYTES)?;
        Ok(ScriptCallReservation {
            method: method.to_owned(),
            reservation,
        })
    }

    pub(crate) fn seal(method: String, args: Vec<ParamValue>, reservation: PayloadReservation, weight: usize) -> Self {
        let values = reservation
            .retain(args, weight)
            .expect("measured arguments fit admitted capacity");
        Self {
            method,
            args: ScriptArguments { values, weight },
        }
    }

    /// Bounded method name.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Original immutable arguments; cloning this handle does not clone parameter data.
    pub fn args(&self) -> &ScriptArguments {
        &self.args
    }
}

impl ScriptCallReservation {
    /// Seal a bounded conversion, including a partial vector when conversion fails.
    /// Failed measurement returns original data to its preparing caller.
    pub fn finish(self, args: Vec<ParamValue>) -> Result<ScriptMethodCall, ScriptCallPreparationError> {
        match argument_weight(&args) {
            Ok(weight) => Ok(ScriptMethodCall::seal(self.method, args, self.reservation, weight)),
            Err(message) => Err(ScriptCallPreparationError {
                method: self.method,
                args,
                message,
            }),
        }
    }
}

#[cfg(test)]
mod tests;
