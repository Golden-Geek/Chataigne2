//! Validated app-owned runtime identity; authored definitions remain untouched.
use super::protocol::ApplicationDefinition;
use std::{ops::Deref, path::Path};

/// Immutable effective launch/control settings, suitable for a shared-runtime map key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApplicationRuntimeKey(ApplicationDefinition);

impl ApplicationRuntimeKey {
    pub fn new(definition: &ApplicationDefinition) -> Result<Self, String> {
        definition.validate()?;
        let mut normalized = definition.clone();
        normalized.executable = definition.executable.as_deref().map(absolute).transpose()?;
        normalized.directory = definition.directory.as_deref().map(absolute).transpose()?;
        if let Some(environment) = &mut normalized.environment {
            // Native launch already rejects duplicate names. Ordering is not environment identity;
            // keep name spelling/value and None versus Some(empty) exactly as authored.
            environment.sort_unstable();
        }
        normalized.validate()?;
        Ok(Self(normalized))
    }

    pub fn definition(&self) -> &ApplicationDefinition {
        &self.0
    }
}

impl Deref for ApplicationRuntimeKey {
    type Target = ApplicationDefinition;
    fn deref(&self) -> &Self::Target {
        self.definition()
    }
}

fn absolute(value: &str) -> Result<String, String> {
    // Validation requires absolute paths. Platform normalization performs no filesystem I/O;
    // verbatim Windows paths retain their distinct semantics, and symlinks are not resolved.
    if !Path::new(value).is_absolute() {
        return Err("runtime paths must be absolute".into());
    }
    let path = std::path::absolute(value).map_err(|error| error.to_string())?;
    let mut text = path.to_str().ok_or("runtime path is not UTF-8")?.to_owned();
    if cfg!(windows) && text.as_bytes().get(1) == Some(&b':') {
        // DOS drive letters are case-insensitive even when a directory enables case sensitivity.
        text.replace_range(..1, &text[..1].to_ascii_uppercase());
    }
    Ok(text)
}

#[cfg(test)]
mod tests;
