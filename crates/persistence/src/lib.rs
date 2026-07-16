//! Durable, app-agnostic file persistence primitives for Golden projects.

#![warn(missing_docs)]

mod file_store;

pub use file_store::{
    RecoveryCandidates, RecoveryJournal, RecoveryPaths, clear_recovery_journal, read_recovery_candidates,
    restore_primary_from_backup, write_file_atomically_with_recovery,
};

#[cfg(test)]
mod file_store_tests;
