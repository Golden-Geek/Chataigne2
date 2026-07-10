//! Versioned project codecs, immutable save snapshots, and crash-safe recovery.

mod codec;
mod store;

pub use codec::{MigrationRegistry, PersistenceError, PersistenceLimits, ProjectCodec, ProjectFile, SaveSnapshot};
pub use store::{LoadedProject, PersistenceStore, RecoverySource};

#[cfg(test)]
mod tests;
