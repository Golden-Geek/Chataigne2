use std::{collections::BTreeMap, sync::Arc};

use thiserror::Error;

use crate::{AlchemistFormula, FormulaId};

#[derive(Clone)]
pub struct CatalogEntry {
    pub formula: Arc<AlchemistFormula>,
    pub built_in: bool,
}

#[derive(Default)]
pub struct FormulaCatalog {
    entries: BTreeMap<FormulaId, CatalogEntry>,
}

impl FormulaCatalog {
    pub fn get(&self, id: FormulaId) -> Option<&CatalogEntry> {
        self.entries.get(&id)
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = (&FormulaId, &CatalogEntry)> {
        self.entries.iter()
    }

    pub fn insert(&mut self, formula: Arc<AlchemistFormula>, built_in: bool) -> Result<(), FormulaCatalogError> {
        let id = formula.id;
        if self.entries.contains_key(&id) {
            return Err(FormulaCatalogError::Duplicate(id));
        }
        self.entries.insert(formula.id, CatalogEntry { formula, built_in });
        Ok(())
    }

    pub fn replace(&mut self, formula: Arc<AlchemistFormula>) -> Result<(), FormulaCatalogError> {
        let id = formula.id;
        let entry = self.entries.get_mut(&id).ok_or(FormulaCatalogError::Missing(id))?;
        if entry.built_in {
            return Err(FormulaCatalogError::ReadOnly(id));
        }
        entry.formula = formula;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum FormulaCatalogError {
    #[error("formula is already registered: {0:?}")]
    Duplicate(FormulaId),
    #[error("formula is not registered: {0:?}")]
    Missing(FormulaId),
    #[error("built-in formula is read-only: {0:?}")]
    ReadOnly(FormulaId),
}
