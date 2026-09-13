//! Authored channel identities and immutable layouts for managed Formula stages.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{ManagedItemId, SocketId, StableRef, ValueComponent, ValueTypeId, component_value_type};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ValueLaneKey(String);

impl ValueLaneKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ChannelLayoutError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ChannelLayoutError::EmptyIdentity);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn input(item: ManagedItemId) -> Self {
        Self(format!("input:{item}"))
    }

    #[must_use]
    pub fn output(item: ManagedItemId, port: &SocketId) -> Self {
        Self(format!("output:{item}:{}:{}", port.as_str().len(), port.as_str()))
    }

    #[must_use]
    pub fn extracted(&self, component: ValueComponent) -> Self {
        Self(format!("extract:{}:{}:{component:?}", self.0.len(), self.0))
    }

    #[must_use]
    pub fn duplicate(item: ManagedItemId, port: &SocketId) -> Self {
        Self(format!("duplicate:{item}:{}:{}", port.as_str().len(), port.as_str()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelProvenance {
    Input(StableRef),
    Derived {
        item: ManagedItemId,
        inputs: Vec<ValueLaneKey>,
    },
    External,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelMetadata {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelDescriptor {
    pub id: ValueLaneKey,
    /// An unresolved source keeps its identity but cannot be type-checked until a schema arrives.
    pub value_type: Option<ValueTypeId>,
    pub port: Option<SocketId>,
    pub label: String,
    pub provenance: ChannelProvenance,
    pub metadata: ChannelMetadata,
}

impl ChannelDescriptor {
    #[must_use]
    pub fn input(
        id: ValueLaneKey,
        label: impl Into<String>,
        source: StableRef,
        value_type: Option<ValueTypeId>,
    ) -> Self {
        Self {
            id,
            value_type,
            port: None,
            label: label.into(),
            provenance: ChannelProvenance::Input(source),
            metadata: ChannelMetadata::default(),
        }
    }

    #[must_use]
    pub fn derived(
        id: ValueLaneKey,
        label: impl Into<String>,
        value_type: ValueTypeId,
        item: ManagedItemId,
        port: SocketId,
        inputs: Vec<ValueLaneKey>,
    ) -> Self {
        Self {
            id,
            value_type: Some(value_type),
            port: Some(port),
            label: label.into(),
            provenance: ChannelProvenance::Derived { item, inputs },
            metadata: ChannelMetadata::default(),
        }
    }

    fn same_structure(&self, other: &Self) -> bool {
        self.id == other.id
            && self.value_type == other.value_type
            && self.port == other.port
            && self.provenance == other.provenance
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelLayout {
    channels: Arc<[ChannelDescriptor]>,
    structural_revision: u64,
    presentation_revision: u64,
}

impl ChannelLayout {
    #[must_use]
    pub fn has_same_structure(&self, other: &Self) -> bool {
        self.channels.len() == other.channels.len()
            && self
                .channels
                .iter()
                .zip(other.channels.iter())
                .all(|(a, b)| a.same_structure(b))
    }

    pub fn new(channels: Vec<ChannelDescriptor>) -> Result<Self, ChannelLayoutError> {
        validate_unique(&channels)?;
        Ok(Self {
            channels: channels.into(),
            structural_revision: 1,
            presentation_revision: 1,
        })
    }

    pub fn reconcile(&self, channels: Vec<ChannelDescriptor>) -> Result<Self, ChannelLayoutError> {
        validate_unique(&channels)?;
        if self.channels.as_ref() == channels.as_slice() {
            return Ok(self.clone());
        }
        let same_structure = self.channels.len() == channels.len()
            && self.channels.iter().zip(&channels).all(|(a, b)| a.same_structure(b));
        Ok(Self {
            channels: channels.into(),
            structural_revision: self.structural_revision + u64::from(!same_structure),
            presentation_revision: self.presentation_revision + 1,
        })
    }

    #[must_use]
    pub fn channels(&self) -> &[ChannelDescriptor] {
        &self.channels
    }

    #[must_use]
    pub fn structural_revision(&self) -> u64 {
        self.structural_revision
    }

    #[must_use]
    pub fn presentation_revision(&self) -> u64 {
        self.presentation_revision
    }

    #[must_use]
    pub fn index_of(&self, id: &ValueLaneKey) -> Option<usize> {
        self.channels.iter().position(|channel| &channel.id == id)
    }

    /// Pack/reduce replacements occupy the first consumed position; untouched channels retain order.
    pub fn replace_selected(
        &self,
        selected: &[usize],
        replacements: Vec<ChannelDescriptor>,
    ) -> Result<Self, ChannelLayoutError> {
        let Some(first) = selected.iter().copied().min() else {
            return Err(ChannelLayoutError::EmptySelection);
        };
        let consumed = validate_indices(selected, self.channels.len())?;
        let mut result = Vec::with_capacity(self.channels.len() - consumed.len() + replacements.len());
        for (index, channel) in self.channels.iter().enumerate() {
            if index == first {
                result.extend(replacements.iter().cloned());
            }
            if !consumed.contains(&index) {
                result.push(channel.clone());
            }
        }
        self.reconcile(result)
    }

    pub fn reorder(&self, order: &[ValueLaneKey]) -> Result<Self, ChannelLayoutError> {
        if order.len() != self.channels.len() {
            return Err(ChannelLayoutError::InvalidReorder);
        }
        let by_id: HashMap<_, _> = self.channels.iter().map(|channel| (&channel.id, channel)).collect();
        let channels = order
            .iter()
            .map(|id| {
                by_id
                    .get(id)
                    .copied()
                    .cloned()
                    .ok_or(ChannelLayoutError::InvalidReorder)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.reconcile(channels)
    }

    /// Resolve a pack or reduction output before any values are evaluated.
    pub fn project_group(
        &self,
        selected: &[ValueLaneKey],
        item: ManagedItemId,
        port: SocketId,
        value_type: ValueTypeId,
        label: impl Into<String>,
    ) -> Result<Self, ChannelLayoutError> {
        let indices = ChannelSelection::Explicit(selected.to_vec())
            .resolve(self, |_| true)?
            .indices;
        self.replace_selected(
            &indices,
            vec![ChannelDescriptor::derived(
                ValueLaneKey::output(item, &port),
                label,
                value_type,
                item,
                port,
                selected.to_vec(),
            )],
        )
    }

    /// Each selected compound is replaced in place by fixed, declared components.
    pub fn project_extract(
        &self,
        selected: &[ValueLaneKey],
        components: &[ValueComponent],
        item: ManagedItemId,
    ) -> Result<Self, ChannelLayoutError> {
        if selected.is_empty() || components.is_empty() {
            return Err(ChannelLayoutError::EmptySelection);
        }
        let indices = ChannelSelection::Explicit(selected.to_vec())
            .resolve(self, |_| true)?
            .indices;
        let selected: HashSet<_> = indices.into_iter().collect();
        let mut channels = Vec::with_capacity(self.channels.len() + selected.len() * components.len());
        for (index, source) in self.channels.iter().enumerate() {
            if !selected.contains(&index) {
                channels.push(source.clone());
                continue;
            }
            let source_type = source
                .value_type
                .as_ref()
                .ok_or_else(|| ChannelLayoutError::UnresolvedChannel(source.id.clone()))?;
            for component in components {
                let value_type = component_value_type(source_type, *component)
                    .ok_or_else(|| ChannelLayoutError::IncompatibleChannel(source.id.clone()))?;
                channels.push(ChannelDescriptor::derived(
                    source.id.extracted(*component),
                    format!("{}.{component:?}", source.label),
                    value_type,
                    item,
                    SocketId::new(format!("{component:?}")),
                    vec![source.id.clone()],
                ));
            }
        }
        self.reconcile(channels)
    }

    /// A duplicate has the authored application/output-port identity, independent of values.
    pub fn project_duplicate(
        &self,
        source: &ValueLaneKey,
        item: ManagedItemId,
        port: SocketId,
    ) -> Result<Self, ChannelLayoutError> {
        let index = self
            .index_of(source)
            .ok_or_else(|| ChannelLayoutError::MissingChannel(source.clone()))?;
        let original = &self.channels[index];
        let mut duplicate = original.clone();
        duplicate.id = ValueLaneKey::duplicate(item, &port);
        duplicate.port = Some(port);
        duplicate.provenance = ChannelProvenance::Derived {
            item,
            inputs: vec![source.clone()],
        };
        let mut channels = self.channels.to_vec();
        channels.insert(index + 1, duplicate);
        self.reconcile(channels)
    }
}

fn validate_unique(channels: &[ChannelDescriptor]) -> Result<(), ChannelLayoutError> {
    let mut seen = HashSet::with_capacity(channels.len());
    for channel in channels {
        if !seen.insert(&channel.id) {
            return Err(ChannelLayoutError::DuplicateIdentity(channel.id.clone()));
        }
    }
    Ok(())
}

fn validate_indices(indices: &[usize], count: usize) -> Result<HashSet<usize>, ChannelLayoutError> {
    let mut seen = HashSet::with_capacity(indices.len());
    for &index in indices {
        if index >= count || !seen.insert(index) {
            return Err(ChannelLayoutError::InvalidSelectionIndex(index));
        }
    }
    Ok(seen)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelSelection {
    AllCompatible,
    Explicit(Vec<ValueLaneKey>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelSelectionResolution {
    pub indices: Vec<usize>,
    /// An empty all-compatible selection is an identity stage with visible authoring status.
    pub no_compatible_channels: bool,
}

impl ChannelSelection {
    pub fn resolve(
        &self,
        layout: &ChannelLayout,
        accepts: impl Fn(&ValueTypeId) -> bool,
    ) -> Result<ChannelSelectionResolution, ChannelLayoutError> {
        match self {
            Self::AllCompatible => {
                let indices: Vec<_> = layout
                    .channels()
                    .iter()
                    .enumerate()
                    .filter(|(_, channel)| channel.value_type.as_ref().is_some_and(&accepts))
                    .map(|(index, _)| index)
                    .collect();
                Ok(ChannelSelectionResolution {
                    no_compatible_channels: indices.is_empty(),
                    indices,
                })
            }
            Self::Explicit(ids) => {
                let mut seen = HashSet::with_capacity(ids.len());
                let indices = ids
                    .iter()
                    .map(|id| {
                        if !seen.insert(id) {
                            return Err(ChannelLayoutError::DuplicateSelection(id.clone()));
                        }
                        let index = layout
                            .index_of(id)
                            .ok_or_else(|| ChannelLayoutError::MissingChannel(id.clone()))?;
                        let channel = &layout.channels()[index];
                        if !channel.value_type.as_ref().is_some_and(&accepts) {
                            return Err(ChannelLayoutError::IncompatibleChannel(id.clone()));
                        }
                        Ok(index)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ChannelSelectionResolution {
                    indices,
                    no_compatible_channels: false,
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelGroups(pub Vec<Vec<ValueLaneKey>>);

impl ChannelGroups {
    pub fn resolve(
        &self,
        layout: &ChannelLayout,
        accepts: impl Fn(&ValueTypeId) -> bool,
    ) -> Result<Vec<Vec<usize>>, ChannelLayoutError> {
        let mut seen = HashSet::new();
        self.0
            .iter()
            .map(|group| {
                if group.is_empty() {
                    return Err(ChannelLayoutError::EmptySelection);
                }
                let resolved = ChannelSelection::Explicit(group.clone()).resolve(layout, &accepts)?;
                for &index in &resolved.indices {
                    if !seen.insert(index) {
                        return Err(ChannelLayoutError::DuplicateSelection(
                            layout.channels()[index].id.clone(),
                        ));
                    }
                }
                Ok(resolved.indices)
            })
            .collect()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChannelLayoutError {
    #[error("channel identity must not be empty")]
    EmptyIdentity,
    #[error("duplicate channel identity `{0:?}`")]
    DuplicateIdentity(ValueLaneKey),
    #[error("selection must contain at least one channel")]
    EmptySelection,
    #[error("invalid or duplicate channel index `{0}`")]
    InvalidSelectionIndex(usize),
    #[error("channel `{0:?}` is missing from the current layout")]
    MissingChannel(ValueLaneKey),
    #[error("channel `{0:?}` is incompatible with this operation")]
    IncompatibleChannel(ValueLaneKey),
    #[error("channel `{0:?}` has no resolved type yet")]
    UnresolvedChannel(ValueLaneKey),
    #[error("channel `{0:?}` is selected more than once")]
    DuplicateSelection(ValueLaneKey),
    #[error("reorder must name each channel exactly once")]
    InvalidReorder,
}
