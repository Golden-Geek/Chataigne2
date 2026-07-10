use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;

use golden_model::Revision;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

pub type DocumentValidator<T> = dyn Fn(&T) -> Result<(), String>;
type Migration = Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistenceLimits {
    pub maximum_file_bytes: u64,
    pub maximum_json_values: usize,
    pub maximum_json_depth: usize,
}

impl Default for PersistenceLimits {
    fn default() -> Self {
        Self {
            maximum_file_bytes: 256 * 1_048_576,
            maximum_json_values: 2_000_000,
            maximum_json_depth: 128,
        }
    }
}

#[derive(Clone)]
pub struct SaveSnapshot<T> {
    pub revision: Revision,
    document: Arc<T>,
}

impl<T> SaveSnapshot<T> {
    pub fn new(revision: Revision, document: T) -> Self {
        Self {
            revision,
            document: Arc::new(document),
        }
    }

    pub fn from_arc(revision: Revision, document: Arc<T>) -> Self {
        Self { revision, document }
    }

    pub fn document(&self) -> &T {
        &self.document
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile<T> {
    pub schema_version: u32,
    pub application_id: SmolStr,
    pub application_version: SmolStr,
    pub revision: Revision,
    pub document: T,
}

#[derive(Default)]
pub struct MigrationRegistry {
    migrations: BTreeMap<u32, Migration>,
}

impl MigrationRegistry {
    pub fn register(
        &mut self,
        from_version: u32,
        migration: impl Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync + 'static,
    ) -> Result<(), PersistenceError> {
        if from_version == 0 || self.migrations.insert(from_version, Box::new(migration)).is_some() {
            return Err(PersistenceError::InvalidMigrationRegistry);
        }
        Ok(())
    }

    fn migrate(
        &self,
        mut version: u32,
        current: u32,
        mut document: serde_json::Value,
    ) -> Result<serde_json::Value, PersistenceError> {
        while version < current {
            let migration = self
                .migrations
                .get(&version)
                .ok_or(PersistenceError::MissingMigration { from: version })?;
            document = migration(document).map_err(PersistenceError::Migration)?;
            version += 1;
        }
        Ok(document)
    }
}

pub struct ProjectCodec<T> {
    application_id: SmolStr,
    application_version: SmolStr,
    current_schema: u32,
    limits: PersistenceLimits,
    migrations: MigrationRegistry,
    marker: PhantomData<fn() -> T>,
}

impl<T> ProjectCodec<T>
where
    T: Clone + Serialize + DeserializeOwned,
{
    pub fn new(
        application_id: impl Into<SmolStr>,
        application_version: impl Into<SmolStr>,
        current_schema: u32,
        limits: PersistenceLimits,
        migrations: MigrationRegistry,
    ) -> Result<Self, PersistenceError> {
        let application_id = application_id.into();
        let application_version = application_version.into();
        if application_id.is_empty() || application_version.is_empty() || current_schema == 0 {
            return Err(PersistenceError::InvalidCodecConfiguration);
        }
        Ok(Self {
            application_id,
            application_version,
            current_schema,
            limits,
            migrations,
            marker: PhantomData,
        })
    }

    pub const fn limits(&self) -> PersistenceLimits {
        self.limits
    }

    pub fn encode(
        &self,
        snapshot: &SaveSnapshot<T>,
        validator: &DocumentValidator<T>,
    ) -> Result<Vec<u8>, PersistenceError> {
        validator(snapshot.document()).map_err(PersistenceError::InvalidDocument)?;
        let file = ProjectFile {
            schema_version: self.current_schema,
            application_id: self.application_id.clone(),
            application_version: self.application_version.clone(),
            revision: snapshot.revision,
            document: snapshot.document().clone(),
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        self.validate_size(bytes.len() as u64)?;
        Ok(bytes)
    }

    pub fn decode(&self, bytes: &[u8], validator: &DocumentValidator<T>) -> Result<SaveSnapshot<T>, PersistenceError> {
        self.validate_size(bytes.len() as u64)?;
        let raw: ProjectFile<serde_json::Value> = serde_json::from_slice(bytes)?;
        if raw.application_id != self.application_id {
            return Err(PersistenceError::WrongApplication {
                expected: self.application_id.clone(),
                actual: raw.application_id,
            });
        }
        if raw.schema_version > self.current_schema || raw.schema_version == 0 {
            return Err(PersistenceError::UnsupportedSchema(raw.schema_version));
        }
        validate_json_shape(&raw.document, self.limits)?;
        let document = self
            .migrations
            .migrate(raw.schema_version, self.current_schema, raw.document)?;
        validate_json_shape(&document, self.limits)?;
        let document = serde_json::from_value(document)?;
        validator(&document).map_err(PersistenceError::InvalidDocument)?;
        Ok(SaveSnapshot::new(raw.revision, document))
    }

    fn validate_size(&self, bytes: u64) -> Result<(), PersistenceError> {
        if bytes > self.limits.maximum_file_bytes {
            Err(PersistenceError::FileTooLarge {
                bytes,
                maximum: self.limits.maximum_file_bytes,
            })
        } else {
            Ok(())
        }
    }
}

fn validate_json_shape(root: &serde_json::Value, limits: PersistenceLimits) -> Result<(), PersistenceError> {
    let mut visited = 0_usize;
    let mut stack = vec![(root, 1_usize)];
    while let Some((value, depth)) = stack.pop() {
        visited += 1;
        if visited > limits.maximum_json_values {
            return Err(PersistenceError::JsonValueLimit);
        }
        if depth > limits.maximum_json_depth {
            return Err(PersistenceError::JsonDepthLimit);
        }
        match value {
            serde_json::Value::Array(values) => {
                stack.extend(values.iter().map(|value| (value, depth + 1)));
            }
            serde_json::Value::Object(values) => {
                stack.extend(values.values().map(|value| (value, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("persistence codec requires non-empty application metadata and a non-zero schema")]
    InvalidCodecConfiguration,
    #[error("migration registry entries must be unique and start at version one")]
    InvalidMigrationRegistry,
    #[error("project belongs to {actual}, expected {expected}")]
    WrongApplication { expected: SmolStr, actual: SmolStr },
    #[error("project schema {0} is not supported")]
    UnsupportedSchema(u32),
    #[error("no migration is registered from schema {from}")]
    MissingMigration { from: u32 },
    #[error("project migration failed: {0}")]
    Migration(String),
    #[error("project validation failed: {0}")]
    InvalidDocument(String),
    #[error("project contains too many JSON values")]
    JsonValueLimit,
    #[error("project JSON nesting is too deep")]
    JsonDepthLimit,
    #[error("project contains {bytes} bytes, exceeding limit {maximum}")]
    FileTooLarge { bytes: u64, maximum: u64 },
    #[error("project and every recorded recovery backup are unreadable")]
    NoRecoverableSnapshot,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
