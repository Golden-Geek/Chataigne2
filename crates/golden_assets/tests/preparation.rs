use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use golden_assets::*;
use sha2::{Digest, Sha256};

const WAIT: Duration = Duration::from_secs(5);

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}

fn binding(path: &str) -> AssetBinding {
    AssetBinding::new(AssetRecord {
        id: AssetId::new(),
        locator: relative(path),
    })
}

fn one_prepared_item() -> PreparationLimits {
    PreparationLimits {
        max_prepared_items: NonZeroUsize::new(1).unwrap(),
        ..Default::default()
    }
}

fn outcome<D: AssetDecoder>(preparer: &AssetPreparer<D>) -> PreparationOutcome<D::Output> {
    let deadline = Instant::now() + WAIT;
    let mut outcomes = Vec::new();
    loop {
        preparer.drain(&mut outcomes, NonZeroUsize::new(16).unwrap());
        if let Some(result) = outcomes.pop() {
            assert!(outcomes.is_empty(), "test expected exactly one outstanding request");
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "asset preparation did not produce an outcome"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

fn bytes(input: &mut AssetInput, maximum_bytes: usize) -> Result<Vec<u8>, PreparationFailure> {
    input.check()?;
    let length = usize::try_from(input.length()).map_err(|_| PreparationFailure::Decode)?;
    if length
        .checked_add(size_of::<Vec<u8>>())
        .is_none_or(|weight| weight > maximum_bytes)
    {
        return Err(PreparationFailure::Decode);
    }
    let mut value = Vec::new();
    value
        .try_reserve_exact(length)
        .map_err(|_| PreparationFailure::Decode)?;
    if value
        .capacity()
        .checked_add(size_of::<Vec<u8>>())
        .is_none_or(|weight| weight > maximum_bytes)
    {
        return Err(PreparationFailure::Decode);
    }
    value.resize(length, 0);
    input
        .read_exact(&mut value)
        .map_err(|error| PreparationFailure::Io(error.kind()))?;
    input.check()?;
    Ok(value)
}

struct BytesDecoder {
    decoded_on: Option<Sender<ThreadId>>,
}

impl AssetDecoder for BytesDecoder {
    type Output = Vec<u8>;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        if let Some(sender) = &self.decoded_on {
            sender.send(thread::current().id()).unwrap();
        }
        bytes(input, maximum_bytes)
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        size_of::<Vec<u8>>() + value.capacity()
    }
}

fn decoder() -> BytesDecoder {
    BytesDecoder { decoded_on: None }
}

#[test]
fn tiny_results_release_decode_scratch_allowance_while_consumers_retain_them() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), b"tiny-result").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let mut preparer = AssetPreparer::new(
        decoder(),
        PreparationLimits {
            max_source_bytes: 4096,
            max_decode_bytes: NonZeroUsize::new(128 * 1024 * 1024).unwrap(),
            max_prepared_bytes: NonZeroUsize::new(128 * 1024 * 1024 + 65536).unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    let first = outcome(&preparer).result.unwrap();
    assert!(first.retained_bytes() < 65536);
    // The pool fits one worst-case decoder, but also two small retained results.
    preparer.submit(scope.capture(&binding)).unwrap();
    let second = outcome(&preparer).result.unwrap();
    assert_eq!(first.value(), second.value());
    assert_eq!(first.retained_bytes(), second.retained_bytes());
    let retirement = preparer.retirement();
    drop(first);
    drop(second);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

#[test]
fn public_consumer_decodes_on_worker_and_duplicate_references_keep_identity_and_fingerprint() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), b"source-data").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let (sent, decoded_on) = mpsc::channel();
    let mut preparer =
        AssetPreparer::new(BytesDecoder { decoded_on: Some(sent) }, PreparationLimits::default()).unwrap();
    let request = preparer.submit(scope.capture(&binding)).unwrap();
    let first = outcome(&preparer);
    assert_eq!(first.request, request);
    assert_eq!(first.asset, binding.record().id);
    let first = first.result.unwrap();
    assert_eq!(first.id(), binding.record().id);
    assert_eq!(first.value(), b"source-data");
    assert!(first.is_current());
    assert_ne!(decoded_on.recv_timeout(WAIT).unwrap(), thread::current().id());

    let request = preparer.submit(scope.capture(&binding)).unwrap();
    let duplicate = outcome(&preparer);
    assert_eq!(duplicate.request, request);
    let duplicate = duplicate.result.unwrap();
    assert_eq!(first.id(), duplicate.id());
    assert_eq!(first.fingerprint(), duplicate.fingerprint());
    assert_eq!(first.value(), duplicate.value());
    let retirement = preparer.retirement();
    drop(first);
    drop(duplicate);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

#[test]
fn project_relocation_and_source_relink_invalidate_captures_without_replacing_asset_identity() {
    let first_project = tempfile::tempdir().unwrap();
    let relocated_project = tempfile::tempdir().unwrap();
    std::fs::write(first_project.path().join("source.bin"), b"before-move").unwrap();
    std::fs::write(relocated_project.path().join("source.bin"), b"after-move").unwrap();
    std::fs::write(relocated_project.path().join("relinked.bin"), b"after-relink").unwrap();
    let mut scope = AssetScope::new(first_project.path()).unwrap();
    let mut binding = binding("source.bin");
    let id = binding.record().id;
    let first = scope.capture(&binding);
    assert_eq!(first.id(), id);
    scope.relocate(relocated_project.path()).unwrap();
    assert!(!first.is_current());
    let relocated = scope.capture(&binding);
    assert_eq!(relocated.id(), id);
    let mut preparer = AssetPreparer::new(decoder(), PreparationLimits::default()).unwrap();
    preparer.submit(relocated.clone()).unwrap();
    let prepared = outcome(&preparer).result.unwrap();
    assert_eq!(prepared.value(), b"after-move");
    assert_eq!(prepared.id(), id);

    binding.relink(relative("relinked.bin"));
    assert_eq!(binding.record().id, id);
    assert!(!relocated.is_current());
    assert!(!prepared.is_current());
    assert!(matches!(
        preparer.submit(relocated),
        Err(PreparationAdmissionError::Stale)
    ));
    preparer.submit(scope.capture(&binding)).unwrap();
    let relinked = outcome(&preparer).result.unwrap();
    assert_eq!(relinked.id(), id);
    assert_eq!(relinked.value(), b"after-relink");
    let retirement = preparer.retirement();
    drop(prepared);
    drop(relinked);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

#[test]
fn missing_directory_and_oversized_sources_return_typed_failures_before_decode() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir(project.path().join("directory")).unwrap();
    std::fs::write(project.path().join("large.bin"), b"too large").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let (sent, decoded_on) = mpsc::channel();
    let mut preparer = AssetPreparer::new(
        BytesDecoder { decoded_on: Some(sent) },
        PreparationLimits {
            max_source_bytes: 4,
            ..Default::default()
        },
    )
    .unwrap();
    for (path, expected) in [
        ("missing.bin", "missing"),
        ("directory", "directory"),
        ("large.bin", "large"),
    ] {
        let binding = binding(path);
        preparer.submit(scope.capture(&binding)).unwrap();
        let result = outcome(&preparer).result;
        assert!(
            match expected {
                "missing" => matches!(&result, Err(PreparationFailure::Missing)),
                "directory" => matches!(&result, Err(PreparationFailure::NotAFile)),
                "large" => matches!(&result, Err(PreparationFailure::SourceTooLarge)),
                _ => unreachable!(),
            },
            "{path}: {result:?}"
        );
    }
    assert!(decoded_on.try_recv().is_err());
    let retirement = preparer.retirement();
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

struct ReplacingDecoder {
    target: PathBuf,
}

impl AssetDecoder for ReplacingDecoder {
    type Output = Vec<u8>;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        let value = bytes(input, maximum_bytes)?;
        let replacement = self.target.with_extension("replacement");
        std::fs::write(&replacement, b"new!").map_err(|error| PreparationFailure::Io(error.kind()))?;
        std::fs::rename(replacement, &self.target).map_err(|error| PreparationFailure::Io(error.kind()))?;
        Ok(value)
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        size_of::<Vec<u8>>() + value.capacity()
    }
}

#[test]
fn atomic_path_replacement_during_decode_rejects_the_old_open_handle_contents() {
    let project = tempfile::tempdir().unwrap();
    let target = project.path().join("source.bin");
    std::fs::write(&target, b"old!").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let mut preparer = AssetPreparer::new(
        ReplacingDecoder { target: target.clone() },
        PreparationLimits::default(),
    )
    .unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    assert!(matches!(
        outcome(&preparer).result,
        Err(PreparationFailure::SourceChanged)
    ));
    assert_eq!(std::fs::read(target).unwrap(), b"new!");
    let retirement = preparer.retirement();
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

struct TransientWriteDecoder {
    target: PathBuf,
}

impl AssetDecoder for TransientWriteDecoder {
    type Output = Vec<u8>;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        std::fs::write(&self.target, b"new!").map_err(|error| PreparationFailure::Io(error.kind()))?;
        let decoded = bytes(input, maximum_bytes);
        std::fs::write(&self.target, b"old!").map_err(|error| PreparationFailure::Io(error.kind()))?;
        decoded
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        size_of::<Vec<u8>>() + value.capacity()
    }
}

#[test]
fn immutable_encoded_input_survives_transient_in_place_writes_and_retains_its_exact_fingerprint() {
    let project = tempfile::tempdir().unwrap();
    let target = project.path().join("source.bin");
    std::fs::write(&target, b"old!").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let mut preparer = AssetPreparer::new(
        TransientWriteDecoder { target: target.clone() },
        PreparationLimits::default(),
    )
    .unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    let prepared = outcome(&preparer).result.unwrap();
    assert_eq!(prepared.value(), b"old!");
    assert_eq!(std::fs::read(target).unwrap(), b"old!");
    assert_eq!(prepared.fingerprint().length, 4);
    let expected: [u8; 32] = Sha256::digest(b"old!").into();
    assert_eq!(prepared.fingerprint().sha256, expected);
    let retirement = preparer.retirement();
    drop(prepared);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

#[derive(Debug, PartialEq, Eq)]
enum Checked {
    Current,
    Cancelled,
    Stale,
    Other,
}

struct ControlledDecoder {
    entered: Sender<()>,
    release: Receiver<()>,
    checked: Sender<Checked>,
}

impl AssetDecoder for ControlledDecoder {
    type Output = Vec<u8>;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        self.entered.send(()).unwrap();
        self.release
            .recv_timeout(WAIT)
            .map_err(|_| PreparationFailure::Decode)?;
        let check = input.check();
        self.checked
            .send(match &check {
                Ok(()) => Checked::Current,
                Err(PreparationFailure::Cancelled) => Checked::Cancelled,
                Err(PreparationFailure::Stale) => Checked::Stale,
                _ => Checked::Other,
            })
            .unwrap();
        check?;
        bytes(input, maximum_bytes)
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        size_of::<Vec<u8>>() + value.capacity()
    }
}

fn controlled() -> (ControlledDecoder, Receiver<()>, Sender<()>, Receiver<Checked>) {
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let (checked, checked_rx) = mpsc::channel();
    (
        ControlledDecoder {
            entered,
            release: release_rx,
            checked,
        },
        entered_rx,
        release,
        checked_rx,
    )
}

#[test]
fn binding_change_during_decode_invalidates_checked_input_and_prevents_prepared_publication() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), b"source").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let (decoder, entered, release, checked) = controlled();
    let mut preparer = AssetPreparer::new(decoder, PreparationLimits::default()).unwrap();
    let capture = scope.capture(&binding);
    preparer.submit(capture.clone()).unwrap();
    entered.recv_timeout(WAIT).unwrap();
    binding.invalidate();
    assert!(!capture.is_current());
    release.send(()).unwrap();
    assert_eq!(checked.recv_timeout(WAIT).unwrap(), Checked::Stale);
    assert!(matches!(outcome(&preparer).result, Err(PreparationFailure::Stale)));
    let retirement = preparer.retirement();
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

#[test]
fn cancelling_a_blocked_decoder_is_nonjoining_and_checked_before_it_resumes_io() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), b"source").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let (decoder, entered, release, checked) = controlled();
    let mut preparer = AssetPreparer::new(decoder, PreparationLimits::default()).unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    entered.recv_timeout(WAIT).unwrap();
    let retirement = preparer.retirement();
    preparer.cancel();
    assert!(!retirement.wait(Duration::from_millis(10)));
    release.send(()).unwrap();
    assert_eq!(checked.recv_timeout(WAIT).unwrap(), Checked::Cancelled);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

#[test]
fn prepared_item_pressure_counts_held_clones_until_every_consumer_releases_them() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), b"source").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let mut preparer = AssetPreparer::new(decoder(), one_prepared_item()).unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    let first = outcome(&preparer).result.unwrap();
    let held = first.clone();
    drop(first);
    preparer.submit(scope.capture(&binding)).unwrap();
    assert!(matches!(
        outcome(&preparer).result,
        Err(PreparationFailure::ResourcePressure)
    ));
    assert_eq!(held.value(), b"source");
    drop(held);

    let deadline = Instant::now() + WAIT;
    let prepared = loop {
        preparer.submit(scope.capture(&binding)).unwrap();
        match outcome(&preparer).result {
            Ok(prepared) => break prepared,
            Err(PreparationFailure::ResourcePressure) => {
                assert!(Instant::now() < deadline, "released payload capacity did not retire");
                thread::sleep(Duration::from_millis(2));
            }
            Err(failure) => panic!("unexpected preparation failure: {failure:?}"),
        }
    };
    let retirement = preparer.retirement();
    drop(prepared);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

#[test]
fn prepared_byte_pressure_rejects_before_decode_even_with_an_available_item_slot() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), vec![0; 512 * 1024]).unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let limits = PreparationLimits {
        max_source_bytes: 512 * 1024,
        max_decode_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        max_prepared_items: NonZeroUsize::new(2).unwrap(),
        max_prepared_bytes: NonZeroUsize::new(1792 * 1024).unwrap(),
        ..Default::default()
    };
    let (sent, decoded_on) = mpsc::channel();
    let mut preparer = AssetPreparer::new(BytesDecoder { decoded_on: Some(sent) }, limits).unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    let held = outcome(&preparer).result.unwrap();
    decoded_on.recv_timeout(WAIT).unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    assert!(matches!(
        outcome(&preparer).result,
        Err(PreparationFailure::ResourcePressure)
    ));
    assert!(
        decoded_on.try_recv().is_err(),
        "byte pressure must be checked before decoder entry"
    );
    let retirement = preparer.retirement();
    drop(held);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

struct BoundedReaderDecoder;

impl AssetDecoder for BoundedReaderDecoder {
    type Output = Vec<u8>;

    fn decode(&mut self, input: &mut AssetInput, _: usize) -> Result<Self::Output, PreparationFailure> {
        assert_eq!(input.length(), 4);
        assert!(input.seek(SeekFrom::Start(5)).is_err());
        assert!(input.seek(SeekFrom::End(-5)).is_err());
        for _ in 0..4 {
            input.rewind().unwrap();
            let mut buffer = [0; 4];
            input.read_exact(&mut buffer).unwrap();
            assert_eq!(&buffer, b"data");
        }
        input.rewind().unwrap();
        assert!(
            input.read(&mut [0]).is_err(),
            "decode reads must not exceed their aggregate byte budget"
        );
        Ok(Vec::new())
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        size_of::<Vec<u8>>() + value.capacity()
    }
}

#[test]
fn public_input_bounds_native_seeks_and_aggregate_decode_reads() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), b"data").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let mut preparer = AssetPreparer::new(
        BoundedReaderDecoder,
        PreparationLimits {
            max_source_bytes: 4,
            ..Default::default()
        },
    )
    .unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    let prepared = outcome(&preparer).result.unwrap();
    let retirement = preparer.retirement();
    drop(prepared);
    drop(preparer);
    assert!(retirement.wait(WAIT));
}

struct Tracked {
    bytes: Vec<u8>,
    dropped_on: Sender<ThreadId>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        let _ = self.dropped_on.send(thread::current().id());
    }
}

struct TrackedDecoder {
    dropped_on: Sender<ThreadId>,
}

impl AssetDecoder for TrackedDecoder {
    type Output = Tracked;

    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure> {
        Ok(Tracked {
            bytes: bytes(input, maximum_bytes)?,
            dropped_on: self.dropped_on.clone(),
        })
    }

    fn retained_bytes(&self, value: &Self::Output) -> usize {
        size_of::<Tracked>() + value.bytes.capacity()
    }
}

#[test]
fn prepared_clones_outlive_preparer_and_final_resource_drop_stays_off_the_consumer() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("source.bin"), b"source").unwrap();
    let scope = AssetScope::new(project.path()).unwrap();
    let binding = binding("source.bin");
    let (dropped_on, dropped) = mpsc::channel();
    let mut preparer = AssetPreparer::new(TrackedDecoder { dropped_on }, one_prepared_item()).unwrap();
    preparer.submit(scope.capture(&binding)).unwrap();
    let prepared = match outcome(&preparer).result {
        Ok(prepared) => prepared,
        Err(failure) => panic!("unexpected preparation failure: {failure:?}"),
    };
    let held = prepared.clone();
    let retirement = preparer.retirement();
    drop(prepared);
    drop(preparer);
    assert!(!retirement.wait(Duration::from_millis(10)));
    assert_eq!(held.value().bytes, b"source");
    assert!(dropped.try_recv().is_err());
    drop(held);
    assert_ne!(dropped.recv_timeout(WAIT).unwrap(), thread::current().id());
    assert!(retirement.wait(WAIT));
}

#[test]
fn relative_scope_roots_are_rejected_without_filesystem_resolution() {
    assert!(AssetScope::new(Path::new("relative-project")).is_err());
}
