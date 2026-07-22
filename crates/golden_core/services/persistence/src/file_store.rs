use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const RECOVERY_JOURNAL_VERSION: u32 = 1;

/// Deterministic sibling paths used for one durable file transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryPaths {
    /// Authoritative destination path.
    pub target: PathBuf,
    /// Last complete contents that preceded the current destination.
    pub backup: PathBuf,
    /// Write-ahead marker retained when a transaction is interrupted.
    pub journal: PathBuf,
}

impl RecoveryPaths {
    /// Builds recovery paths without changing the destination extension.
    pub fn for_target(path: impl AsRef<Path>) -> io::Result<Self> {
        let target = path.as_ref().to_path_buf();
        let file_name = target.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("persistence target has no file name: {}", target.display()),
            )
        })?;
        Ok(Self {
            backup: sibling_with_suffix(&target, file_name, ".backup"),
            journal: sibling_with_suffix(&target, file_name, ".recovery.json"),
            target,
        })
    }
}

/// Versioned write-ahead record for an in-progress atomic replacement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryJournal {
    /// Journal schema version.
    pub schema_version: u32,
    /// Destination file name within the journal directory.
    pub target_file: String,
    /// Backup file name when a previous complete destination existed.
    pub backup_file: Option<String>,
    /// Digest of the new contents intended for the destination.
    pub pending_sha256: String,
    /// Digest of the previous complete contents copied to the backup.
    pub backup_sha256: Option<String>,
}

/// Primary and backup bytes available to a recovery-aware decoder.
#[derive(Clone, Debug)]
pub struct RecoveryCandidates {
    /// Deterministic transaction paths.
    pub paths: RecoveryPaths,
    /// Current destination bytes, when readable.
    pub primary: Option<Vec<u8>>,
    /// Previous complete destination bytes, when readable.
    pub backup: Option<Vec<u8>>,
    /// Read error for the primary when the backup kept recovery possible.
    pub primary_error: Option<String>,
    /// Read error for the backup when the primary remained available.
    pub backup_error: Option<String>,
    /// Parsed pending journal, when one exists and is valid.
    pub journal: Option<RecoveryJournal>,
    /// Journal parse/read error that should be surfaced in recovery diagnostics.
    pub journal_error: Option<String>,
}

/// Replaces a file atomically while retaining its previous complete contents and a write-ahead journal.
pub fn write_file_atomically_with_recovery(path: impl AsRef<Path>, contents: &[u8]) -> io::Result<RecoveryPaths> {
    let paths = RecoveryPaths::for_target(path)?;
    if let Some(parent) = paths.target.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let previous = match fs::read(&paths.target) {
        Ok(previous) => Some(previous),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if let Some(previous) = previous.as_deref() {
        atomic_write(&paths.backup, previous)?;
    } else {
        remove_if_present(&paths.backup)?;
    }

    let journal = RecoveryJournal {
        schema_version: RECOVERY_JOURNAL_VERSION,
        target_file: display_file_name(&paths.target),
        backup_file: previous.as_ref().map(|_| display_file_name(&paths.backup)),
        pending_sha256: sha256_hex(contents),
        backup_sha256: previous.as_deref().map(sha256_hex),
    };
    let journal_bytes = serde_json::to_vec_pretty(&journal).map_err(io::Error::other)?;
    atomic_write(&paths.journal, &journal_bytes)?;
    atomic_write(&paths.target, contents)?;
    remove_if_present(&paths.journal)?;
    Ok(paths)
}

/// Reads the destination, its last complete backup, and any interrupted-transaction journal.
pub fn read_recovery_candidates(path: impl AsRef<Path>) -> io::Result<RecoveryCandidates> {
    let paths = RecoveryPaths::for_target(path)?;
    let (primary, primary_error) = read_optional(&paths.target)?;
    let (backup, backup_error) = read_optional(&paths.backup)?;
    if primary.is_none() && backup.is_none() {
        let diagnostic = primary_error
            .as_deref()
            .or(backup_error.as_deref())
            .unwrap_or("both files are absent");
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "neither project file nor recovery backup is readable: {} ({diagnostic})",
                paths.target.display(),
            ),
        ));
    }

    let (journal, journal_error) = match fs::read(&paths.journal) {
        Ok(bytes) => match serde_json::from_slice::<RecoveryJournal>(&bytes) {
            Ok(journal) if journal.schema_version == RECOVERY_JOURNAL_VERSION => (Some(journal), None),
            Ok(journal) => (
                None,
                Some(format!(
                    "unsupported recovery journal version {}",
                    journal.schema_version
                )),
            ),
            Err(error) => (None, Some(format!("invalid recovery journal: {error}"))),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => (None, None),
        Err(error) => (None, Some(format!("failed to read recovery journal: {error}"))),
    };

    Ok(RecoveryCandidates {
        paths,
        primary,
        backup,
        primary_error,
        backup_error,
        journal,
        journal_error,
    })
}

/// Atomically repairs a failed primary from a backup that the caller has already validated.
pub fn restore_primary_from_backup(paths: &RecoveryPaths, backup: &[u8]) -> io::Result<()> {
    let journal = RecoveryJournal {
        schema_version: RECOVERY_JOURNAL_VERSION,
        target_file: display_file_name(&paths.target),
        backup_file: Some(display_file_name(&paths.backup)),
        pending_sha256: sha256_hex(backup),
        backup_sha256: Some(sha256_hex(backup)),
    };
    let journal_bytes = serde_json::to_vec_pretty(&journal).map_err(io::Error::other)?;
    atomic_write(&paths.journal, &journal_bytes)?;
    atomic_write(&paths.target, backup)?;
    remove_if_present(&paths.journal)
}

/// Removes a stale write-ahead journal after the primary destination is verified.
pub fn clear_recovery_journal(path: impl AsRef<Path>) -> io::Result<()> {
    let paths = RecoveryPaths::for_target(path)?;
    remove_if_present(&paths.journal)
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(contents)?;
    file.commit()
}

fn read_optional(path: &Path) -> io::Result<(Option<Vec<u8>>, Option<String>)> {
    match fs::read(path) {
        Ok(bytes) => Ok((Some(bytes), None)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((None, None)),
        Err(error) => Ok((None, Some(error.to_string()))),
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn sibling_with_suffix(target: &Path, file_name: &std::ffi::OsStr, suffix: &str) -> PathBuf {
    let mut sibling_name = OsString::from(file_name);
    sibling_name.push(suffix);
    target.with_file_name(sibling_name)
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn sha256_hex(contents: &[u8]) -> String {
    format!("{:x}", Sha256::digest(contents))
}
