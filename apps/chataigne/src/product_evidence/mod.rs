mod digest;
mod engine_helpers;
mod osc_loopback;

#[cfg(test)]
mod tests;

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};

use serde_json::{json, Value};

pub(crate) const OSC_LOOPBACK_SCENARIO: &str = "phase0.osc-loopback.v1";
const RESULT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    scenario: String,
}

pub(crate) fn try_run_from_env() -> io::Result<bool> {
    let invocation = match parse_invocation(std::env::args_os().skip(1)) {
        Ok(Some(invocation)) => invocation,
        Ok(None) => return Ok(false),
        Err(error) => {
            emit_failure("invalid", &error);
            return Err(io::Error::new(io::ErrorKind::InvalidInput, error));
        }
    };

    match run_named_scenario(&invocation.scenario) {
        Ok(evidence) => {
            let semantic_digest = match digest::semantic_digest(&evidence) {
                Ok(digest) => digest,
                Err(error) => {
                    emit_failure(&invocation.scenario, &error);
                    return Err(io::Error::other(error));
                }
            };
            emit(json!({
                "event": "chataigne.product_evidence.result",
                "schema_version": RESULT_SCHEMA_VERSION,
                "scenario": invocation.scenario,
                "status": "pass",
                "semantic_digest": semantic_digest,
                "evidence": evidence,
            }));
            Ok(true)
        }
        Err(error) => {
            emit_failure(&invocation.scenario, &error);
            Err(io::Error::other(format!(
                "product evidence scenario '{}' failed: {error}",
                invocation.scenario
            )))
        }
    }
}

fn run_named_scenario(scenario: &str) -> Result<Value, String> {
    match scenario {
        OSC_LOOPBACK_SCENARIO => osc_loopback::run(),
        _ => Err(format!(
            "unknown product evidence scenario '{scenario}'; available scenario: {OSC_LOOPBACK_SCENARIO}"
        )),
    }
}

fn parse_invocation<I>(args: I) -> Result<Option<Invocation>, String>
where
    I: IntoIterator<Item = OsString>,
{
    let args = args.into_iter().collect::<Vec<_>>();
    let mut scenario = None::<String>;
    let mut consumed = vec![false; args.len()];

    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == OsStr::new("--product-evidence") {
            if scenario.is_some() {
                return Err("--product-evidence may be supplied only once".to_string());
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| "--product-evidence requires a scenario name".to_string())?;
            scenario = Some(parse_scenario_name(value)?);
            consumed[index] = true;
            consumed[index + 1] = true;
            index += 2;
            continue;
        }

        if let Some(argument) = argument.to_str() {
            if let Some(value) = argument.strip_prefix("--product-evidence=") {
                if scenario.is_some() {
                    return Err("--product-evidence may be supplied only once".to_string());
                }
                if value.is_empty() {
                    return Err("--product-evidence requires a scenario name".to_string());
                }
                scenario = Some(value.to_string());
                consumed[index] = true;
            }
        }
        index += 1;
    }

    let Some(scenario) = scenario else {
        return Ok(None);
    };
    let incompatible = args
        .iter()
        .zip(consumed)
        .filter_map(|(argument, was_consumed)| (!was_consumed).then_some(argument))
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if !incompatible.is_empty() {
        return Err(format!(
            "--product-evidence is exclusive and cannot be combined with: {}",
            incompatible.join(" ")
        ));
    }

    Ok(Some(Invocation { scenario }))
}

fn parse_scenario_name(value: &OsStr) -> Result<String, String> {
    value
        .to_str()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "product evidence scenario names must be non-empty UTF-8".to_string())
}

fn emit_failure(scenario: &str, error: &str) {
    emit(json!({
        "event": "chataigne.product_evidence.result",
        "schema_version": RESULT_SCHEMA_VERSION,
        "scenario": scenario,
        "status": "fail",
        "error": error,
    }));
}

fn emit(value: Value) {
    println!("{value}");
    let _ = io::stdout().flush();
}
