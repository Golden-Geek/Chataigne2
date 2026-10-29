use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

use golden_io::WorkerCancellation;
use sha2::{Digest, Sha256};

use crate::{AssetCapture, PreparationFailure};

/// Disposable content observation, never an authored identity or a native handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceFingerprint {
    pub length: u64,
    pub sha256: [u8; 32],
}

/// Worker-local immutable encoded source. Staging/hashing and all reads/seeks check cancellation
/// and source lifetime. A blocking OS read retains its worker slot and staging reservation.
pub struct AssetInput {
    bytes: Cursor<Box<[u8]>>,
    fingerprint: SourceFingerprint,
    capture: AssetCapture,
    cancel: WorkerCancellation,
    length: u64,
    remaining: u64,
}

impl AssetInput {
    pub(crate) fn open(
        capture: AssetCapture,
        cancel: WorkerCancellation,
        maximum: u64,
    ) -> Result<Self, PreparationFailure> {
        check(&capture, &cancel)?;
        let path = capture.resolve();
        // Windows rejects opening a directory before handle metadata is available. Preflight
        // also avoids treating known non-file endpoints as decoder input; recheck the handle.
        let metadata = std::fs::metadata(&path).map_err(PreparationFailure::from)?;
        if !metadata.is_file() {
            return Err(PreparationFailure::NotAFile);
        }
        if metadata.len() > maximum {
            return Err(PreparationFailure::SourceTooLarge);
        }
        let mut file = File::open(path).map_err(PreparationFailure::from)?;
        let metadata = file.metadata().map_err(PreparationFailure::from)?;
        if !metadata.is_file() {
            return Err(PreparationFailure::NotAFile);
        }
        if metadata.len() > maximum {
            return Err(PreparationFailure::SourceTooLarge);
        }
        let length = usize::try_from(metadata.len()).map_err(|_| PreparationFailure::SourceTooLarge)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| PreparationFailure::ResourcePressure)?;
        bytes.resize(length, 0);
        let mut digest = Sha256::new();
        let mut filled = 0;
        while filled < length {
            check(&capture, &cancel)?;
            let end = filled.saturating_add(64 * 1024).min(length);
            let count = file.read(&mut bytes[filled..end]).map_err(PreparationFailure::from)?;
            if count == 0 {
                return Err(PreparationFailure::SourceChanged);
            }
            digest.update(&bytes[filled..filled + count]);
            filled += count;
        }
        check(&capture, &cancel)?;
        if file.read(&mut [0]).map_err(PreparationFailure::from)? != 0 {
            return Err(PreparationFailure::SourceChanged);
        }
        let fingerprint = SourceFingerprint {
            length: length as u64,
            sha256: digest.finalize().into(),
        };
        Ok(Self {
            bytes: Cursor::new(bytes.into_boxed_slice()),
            fingerprint,
            capture,
            cancel,
            length: length as u64,
            remaining: maximum.saturating_mul(4),
        })
    }

    pub fn length(&self) -> u64 {
        self.length
    }
    pub fn check(&self) -> Result<(), PreparationFailure> {
        check(&self.capture, &self.cancel)
    }

    pub(crate) fn fingerprint(&self) -> Result<SourceFingerprint, PreparationFailure> {
        self.check()?;
        Ok(self.fingerprint)
    }
}

pub(crate) fn check(capture: &AssetCapture, cancel: &WorkerCancellation) -> Result<(), PreparationFailure> {
    if cancel.is_cancelled() {
        Err(PreparationFailure::Cancelled)
    } else if !capture.is_current() {
        Err(PreparationFailure::Stale)
    } else {
        Ok(())
    }
}

impl Read for AssetInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.check().map_err(io::Error::other)?;
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::other("asset decode read budget exhausted"));
        }
        let capacity = buffer.len().min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let count = self.bytes.read(&mut buffer[..capacity])?;
        self.remaining -= count as u64;
        Ok(count)
    }
}

impl Seek for AssetInput {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.check().map_err(io::Error::other)?;
        let target = match position {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(offset) => self.length.checked_add_signed(offset),
            SeekFrom::Current(offset) => self.bytes.position().checked_add_signed(offset),
        }
        .filter(|target| *target <= self.length)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "asset seek exceeds admitted source"))?;
        self.bytes.seek(SeekFrom::Start(target))
    }
}
