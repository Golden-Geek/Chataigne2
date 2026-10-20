use std::{
    ops::Deref,
    sync::{Arc, LazyLock},
};

use serde::{Deserialize, Serialize};

use crate::ParameterEnumOption;

/// Immutable enum-domain storage shared by live state, snapshots and history.
///
/// Keeping the Vec behind an Arc also lets native producers protect the same
/// allocation through a retained-payload owner, including exported snapshots.
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct ParameterEnumOptions(Arc<Vec<ParameterEnumOption>>);

impl ParameterEnumOptions {
    /// Adopts a shared allocation without copying its entries or changing identity.
    pub fn from_shared(options: Arc<Vec<ParameterEnumOption>>) -> Self {
        Self(options)
    }

    /// Shared backing storage, for native lifetime ownership and identity checks.
    pub fn shared(&self) -> &Arc<Vec<ParameterEnumOption>> {
        &self.0
    }

    /// Whether two snapshots share the same immutable domain.
    pub fn same_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Borrowed options in their authored order.
    pub fn as_slice(&self) -> &[ParameterEnumOption] {
        self.0.as_slice()
    }

    /// Whether this domain has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Backing allocation capacity, including spare slots.
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl Default for ParameterEnumOptions {
    fn default() -> Self {
        static EMPTY: LazyLock<Arc<Vec<ParameterEnumOption>>> = LazyLock::new(|| Arc::new(Vec::new()));
        Self(EMPTY.clone())
    }
}

impl From<Vec<ParameterEnumOption>> for ParameterEnumOptions {
    fn from(options: Vec<ParameterEnumOption>) -> Self {
        if options.capacity() == 0 {
            Self::default()
        } else {
            Self(Arc::new(options))
        }
    }
}

impl<'de> Deserialize<'de> for ParameterEnumOptions {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<ParameterEnumOption>::deserialize(deserializer).map(Self::from)
    }
}

impl FromIterator<ParameterEnumOption> for ParameterEnumOptions {
    fn from_iter<T: IntoIterator<Item = ParameterEnumOption>>(iter: T) -> Self {
        Vec::from_iter(iter).into()
    }
}

impl Deref for ParameterEnumOptions {
    type Target = [ParameterEnumOption];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a ParameterEnumOptions {
    type Item = &'a ParameterEnumOption;
    type IntoIter = std::slice::Iter<'a, ParameterEnumOption>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl PartialEq for ParameterEnumOptions {
    fn eq(&self, other: &Self) -> bool {
        self.same_storage(other) || self.as_slice() == other.as_slice()
    }
}

#[cfg(test)]
mod tests;
