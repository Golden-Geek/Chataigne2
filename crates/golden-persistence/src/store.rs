use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use atomic_write_file::AtomicWriteFile;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::codec::DocumentValidator;
use crate::{PersistenceError, ProjectCodec, SaveSnapshot};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoverySource {
    Primary,
    Backup(PathBuf),
}

pub struct LoadedProject<T> {
    pub snapshot: SaveSnapshot<T>,
    pub source: RecoverySource,
    pub primary_error: Option<String>,
}

pub struct PersistenceStore {
    maximum_backups: usize,
}

impl PersistenceStore {
    pub fn new(maximum_backups: usize) -> Result<Self, PersistenceError> {
        if maximum_backups == 0 {
            return Err(PersistenceError::InvalidCodecConfiguration);
        }
        Ok(Self { maximum_backups })
    }

    pub fn save<T>(
        &self,
        path: &Path,
        codec: &ProjectCodec<T>,
        snapshot: &SaveSnapshot<T>,
        validator: &DocumentValidator<T>,
    ) -> Result<(), PersistenceError>
    where
        T: Clone + Serialize + DeserializeOwned,
    {
        let bytes = codec.encode(snapshot, validator)?;
        let journal_path = sidecar_path(path, ".recovery.json")?;
        let mut journal = self.read_journal(&journal_path, codec.limits().maximum_file_bytes)?;
        if path.exists() {
            let previous = read_limited(path, codec.limits().maximum_file_bytes)?;
            let backup_path = sidecar_path(path, &format!(".backup.{}", journal.next_sequence))?;
            atomic_write(&backup_path, &previous)?;
            journal.backups.push(BackupEntry {
                sequence: journal.next_sequence,
            });
            journal.next_sequence = journal.next_sequence.saturating_add(1);
            atomic_write(&journal_path, &serde_json::to_vec_pretty(&journal)?)?;
        }
        atomic_write(path, &bytes)?;
        while journal.backups.len() > self.maximum_backups {
            let stale = journal.backups.remove(0);
            let stale_path = sidecar_path(path, &format!(".backup.{}", stale.sequence))?;
            match fs::remove_file(stale_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        atomic_write(&journal_path, &serde_json::to_vec_pretty(&journal)?)?;
        Ok(())
    }

    pub fn load_or_recover<T>(
        &self,
        path: &Path,
        codec: &ProjectCodec<T>,
        validator: &DocumentValidator<T>,
    ) -> Result<LoadedProject<T>, PersistenceError>
    where
        T: Clone + Serialize + DeserializeOwned,
    {
        match read_limited(path, codec.limits().maximum_file_bytes).and_then(|bytes| codec.decode(&bytes, validator)) {
            Ok(snapshot) => {
                return Ok(LoadedProject {
                    snapshot,
                    source: RecoverySource::Primary,
                    primary_error: None,
                });
            }
            Err(primary_error) => {
                let primary_error = primary_error.to_string();
                let journal_path = sidecar_path(path, ".recovery.json")?;
                let journal = self.read_journal(&journal_path, codec.limits().maximum_file_bytes)?;
                for backup in journal.backups.iter().rev() {
                    let backup_path = sidecar_path(path, &format!(".backup.{}", backup.sequence))?;
                    let candidate = read_limited(&backup_path, codec.limits().maximum_file_bytes)
                        .and_then(|bytes| codec.decode(&bytes, validator));
                    if let Ok(snapshot) = candidate {
                        return Ok(LoadedProject {
                            snapshot,
                            source: RecoverySource::Backup(backup_path),
                            primary_error: Some(primary_error),
                        });
                    }
                }
            }
        }
        Err(PersistenceError::NoRecoverableSnapshot)
    }

    fn read_journal(&self, path: &Path, maximum_bytes: u64) -> Result<RecoveryJournal, PersistenceError> {
        if !path.exists() {
            return Ok(RecoveryJournal::default());
        }
        let bytes = read_limited(path, maximum_bytes)?;
        let journal: RecoveryJournal = serde_json::from_slice(&bytes)?;
        if journal.schema_version != 1 {
            return Err(PersistenceError::UnsupportedSchema(journal.schema_version));
        }
        Ok(journal)
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    file.commit()?;
    Ok(())
}

fn read_limited(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, PersistenceError> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(maximum_bytes.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(PersistenceError::FileTooLarge {
            bytes: bytes.len() as u64,
            maximum: maximum_bytes,
        });
    }
    Ok(bytes)
}

fn sidecar_path(path: &Path, suffix: &str) -> Result<PathBuf, PersistenceError> {
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "project path has no file name"))?;
    let mut sidecar_name = OsString::from(file_name);
    sidecar_name.push(suffix);
    Ok(path.with_file_name(sidecar_name))
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupEntry {
    sequence: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecoveryJournal {
    schema_version: u32,
    next_sequence: u64,
    backups: Vec<BackupEntry>,
}

impl Default for RecoveryJournal {
    fn default() -> Self {
        Self {
            schema_version: 1,
            next_sequence: 1,
            backups: Vec::new(),
        }
    }
}
