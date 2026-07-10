//! Language-neutral script surface declarations and validation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExecutionClass {
    Deterministic,
    TimeDependent,
    Effectful,
    AsyncIo,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptMember {
    pub name: SmolStr,
    pub execution: ExecutionClass,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleScriptSurface {
    pub module_type: SmolStr,
    pub methods: Vec<ScriptMember>,
    pub callbacks: Vec<SmolStr>,
    pub template: String,
}

#[derive(Default)]
pub struct ScriptSurfaceRegistry {
    surfaces: BTreeMap<SmolStr, ModuleScriptSurface>,
}

impl ScriptSurfaceRegistry {
    pub fn register(&mut self, surface: ModuleScriptSurface) -> Result<(), ScriptSurfaceError> {
        validate_surface(&surface)?;
        if self.surfaces.contains_key(&surface.module_type) {
            return Err(ScriptSurfaceError::DuplicateModule(surface.module_type));
        }
        self.surfaces.insert(surface.module_type.clone(), surface);
        Ok(())
    }

    pub fn get(&self, module_type: &str) -> Option<&ModuleScriptSurface> {
        self.surfaces.get(module_type)
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}

fn validate_surface(surface: &ModuleScriptSurface) -> Result<(), ScriptSurfaceError> {
    if surface.module_type.is_empty() || surface.template.trim().is_empty() {
        return Err(ScriptSurfaceError::IncompleteSurface);
    }
    let mut names = BTreeSet::new();
    for method in &surface.methods {
        if method.name.is_empty() || !names.insert(method.name.clone()) {
            return Err(ScriptSurfaceError::DuplicateMember(method.name.clone()));
        }
    }
    for callback in &surface.callbacks {
        if callback.is_empty() || !names.insert(callback.clone()) {
            return Err(ScriptSurfaceError::DuplicateMember(callback.clone()));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ScriptSurfaceError {
    #[error("script surface must have a module type and non-empty template")]
    IncompleteSurface,
    #[error("script module is already registered: {0}")]
    DuplicateModule(SmolStr),
    #[error("script member is empty or duplicated: {0}")]
    DuplicateMember(SmolStr),
}

#[cfg(test)]
mod tests;
