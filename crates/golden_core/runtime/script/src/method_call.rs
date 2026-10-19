use golden_io::{PayloadPool, PayloadReservation, RetainedPayload};
use golden_parameters::ParamValue;
use std::{mem::size_of, ops::Deref, sync::Mutex};

const MAX_ARGUMENTS: usize = 4096;
const MAX_ARGUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_METHOD_BYTES: usize = 1024;

fn pool() -> Result<PayloadPool, &'static str> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("script-method-input", 4096, 64 * 1024 * 1024)
                .map_err(|_| "script argument disposer unavailable")?,
        );
    }
    Ok(pool.as_ref().expect("initialized argument pool").clone())
}

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
        if let ParamValue::Reference(reference) = arg
            && reference.relative_path_from_root.len() > MAX_ARGUMENTS
        {
            return Err("script reference exceeds 4096 path segments");
        }
        weight = weight.saturating_add(arg.retained_bytes().saturating_sub(size_of::<ParamValue>()));
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
            let reservation = pool()?
                .try_reserve(weight)
                .map_err(|_| "script argument lifetime capacity exhausted")?;
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
        let reservation = pool()?
            .try_reserve(MAX_ARGUMENT_BYTES)
            .map_err(|_| "script argument lifetime capacity exhausted")?;
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
