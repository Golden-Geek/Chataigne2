use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Mutex;

use super::*;

#[derive(Debug, Default)]
struct TestIo {
    inputs: Mutex<VecDeque<u32>>,
    outputs: Mutex<Vec<u32>>,
}

impl TestIo {
    fn with_inputs(inputs: impl IntoIterator<Item = u32>) -> Self {
        Self {
            inputs: Mutex::new(inputs.into_iter().collect()),
            outputs: Mutex::default(),
        }
    }
}

impl ModuleIo for TestIo {
    type Input = u32;
    type Output = u32;
    type Error = Infallible;

    fn next_input(&self) -> Result<Option<Self::Input>, Self::Error> {
        Ok(self.inputs.lock().expect("inputs mutex").pop_front())
    }

    fn dispatch_output(&self, output: AuthoritativeOutput<Self::Output>) -> Result<(), Self::Error> {
        self.outputs.lock().expect("outputs mutex").push(output.into_inner());
        Ok(())
    }
}

#[test]
fn recording_io_preserves_deterministic_order_and_authoritative_outputs() {
    let io = RecordingModuleIo::new(TestIo::with_inputs([7, 9]), DeterministicIoClock::new(100, 5));
    let facades = ApplicationFacades::new((), (), (), (), io, (), ());

    assert_eq!(facades.module_io().next_input().unwrap(), Some(7));
    facades
        .module_io()
        .dispatch_output(facades.authorize_output(11))
        .unwrap();
    assert_eq!(facades.module_io().next_input().unwrap(), Some(9));

    let recording = facades.module_io().recording();
    assert_eq!(recording.schema_version, 1);
    assert_eq!(
        recording.inputs[0],
        RecordedInput {
            sequence: 0,
            timestamp_ns: 100,
            payload: 7
        }
    );
    assert_eq!(
        recording.outputs[0],
        RecordedOutput {
            sequence: 1,
            timestamp_ns: 105,
            payload: 11
        }
    );
    assert_eq!(
        recording.inputs[1],
        RecordedInput {
            sequence: 2,
            timestamp_ns: 110,
            payload: 9
        }
    );
}

#[test]
fn matching_shadow_returns_only_authoritative_output() {
    let executor = ShadowExecutor::new(
        |value: &u32| Ok::<_, Infallible>(value * 2),
        Some(|value: &u32| Ok::<_, Infallible>(value + value)),
        JsonSemanticDigester,
    );

    let run = executor.run(&21).unwrap();

    assert_eq!(run.output, 42);
    assert_eq!(run.comparison.status, ShadowStatus::Match);
    assert_eq!(run.comparison.authoritative_digest, run.comparison.shadow_digest);
}

#[test]
fn mismatching_shadow_never_replaces_authoritative_output() {
    let executor = ShadowExecutor::new(
        |value: &u32| Ok::<_, Infallible>(value * 2),
        Some(|value: &u32| Ok::<_, Infallible>(value * 3)),
        JsonSemanticDigester,
    );

    let run = executor.run(&10).unwrap();

    assert_eq!(run.output, 20);
    assert_eq!(run.comparison.status, ShadowStatus::Mismatch);
    assert_ne!(run.comparison.authoritative_digest, run.comparison.shadow_digest);
}

#[test]
fn shadow_failure_is_observational() {
    let executor = ShadowExecutor::new(
        |value: &u32| Ok::<_, Infallible>(value * 2),
        Some(|_: &u32| Err::<u32, _>("shadow failed")),
        JsonSemanticDigester,
    );

    let run = executor.run(&5).unwrap();

    assert_eq!(run.output, 10);
    assert_eq!(run.comparison.status, ShadowStatus::ShadowFailed);
    assert_eq!(run.comparison.diagnostic.as_deref(), Some("shadow failed"));
}

#[test]
fn semantic_digest_is_stable() {
    let first = JsonSemanticDigester.digest(&vec![1_u32, 2, 3]).unwrap();
    let second = JsonSemanticDigester.digest(&vec![1_u32, 2, 3]).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.to_hex().len(), 64);
}
