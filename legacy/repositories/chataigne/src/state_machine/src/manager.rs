use indexmap::IndexMap;
use uuid::Uuid;

use crate::{Processor, ProcessorId};

macro_rules! manager_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

manager_id!(ProcessorManagerId);
manager_id!(ProcessorGroupId);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProcessorExecutionPolicy {
    #[default]
    InsertionOrder,
    ReverseInsertionOrder,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProcessorGroup {
    pub id: ProcessorGroupId,
    pub label: String,
    pub processors: IndexMap<ProcessorId, Processor>,
    pub execution_policy: ProcessorExecutionPolicy,
    pub enabled: bool,
}

impl ProcessorGroup {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: ProcessorGroupId::new(),
            label: label.into(),
            processors: IndexMap::new(),
            execution_policy: ProcessorExecutionPolicy::default(),
            enabled: true,
        }
    }

    pub fn add_processor(&mut self, processor: Processor) -> Result<ProcessorId, ProcessorManagerError> {
        let id = processor.id;
        if self.processors.contains_key(&id) {
            return Err(ProcessorManagerError::DuplicateProcessor(id));
        }
        self.processors.insert(id, processor);
        Ok(id)
    }

    #[must_use]
    pub fn active_processor_ids(&self) -> Vec<ProcessorId> {
        if !self.enabled {
            return Vec::new();
        }
        ordered_ids(
            self.processors
                .values()
                .filter(|processor| processor.enabled)
                .map(|processor| processor.id),
            self.execution_policy,
        )
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProcessorManager {
    pub id: ProcessorManagerId,
    pub processors: IndexMap<ProcessorId, Processor>,
    pub groups: IndexMap<ProcessorGroupId, ProcessorGroup>,
    pub execution_policy: ProcessorExecutionPolicy,
    pub enabled: bool,
}

impl Default for ProcessorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessorManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: ProcessorManagerId::new(),
            processors: IndexMap::new(),
            groups: IndexMap::new(),
            execution_policy: ProcessorExecutionPolicy::default(),
            enabled: true,
        }
    }

    pub fn add_processor(&mut self, processor: Processor) -> Result<ProcessorId, ProcessorManagerError> {
        let id = processor.id;
        self.ensure_processor_is_unique(id)?;
        self.processors.insert(id, processor);
        Ok(id)
    }

    pub fn add_group(&mut self, group: ProcessorGroup) -> Result<ProcessorGroupId, ProcessorManagerError> {
        if self.groups.contains_key(&group.id) {
            return Err(ProcessorManagerError::DuplicateGroup(group.id));
        }
        for processor in group.processors.values() {
            self.ensure_processor_is_unique(processor.id)?;
        }
        let id = group.id;
        self.groups.insert(id, group);
        Ok(id)
    }

    pub fn add_group_processor(
        &mut self,
        group: ProcessorGroupId,
        processor: Processor,
    ) -> Result<ProcessorId, ProcessorManagerError> {
        self.ensure_processor_is_unique(processor.id)?;
        self.groups
            .get_mut(&group)
            .ok_or(ProcessorManagerError::MissingGroup(group))?
            .add_processor(processor)
    }

    pub fn processors(&self) -> impl Iterator<Item = &Processor> {
        self.processors
            .values()
            .chain(self.groups.values().flat_map(|group| group.processors.values()))
    }

    #[must_use]
    pub fn processor(&self, id: ProcessorId) -> Option<&Processor> {
        self.processors
            .get(&id)
            .or_else(|| self.groups.values().find_map(|group| group.processors.get(&id)))
    }

    #[must_use]
    pub fn active_processor_ids(&self) -> Vec<ProcessorId> {
        if !self.enabled {
            return Vec::new();
        }
        let direct = ordered_ids(
            self.processors
                .values()
                .filter(|processor| processor.enabled)
                .map(|processor| processor.id),
            self.execution_policy,
        );
        direct
            .into_iter()
            .chain(self.groups.values().flat_map(ProcessorGroup::active_processor_ids))
            .collect()
    }

    fn ensure_processor_is_unique(&self, id: ProcessorId) -> Result<(), ProcessorManagerError> {
        if self.processor(id).is_some() {
            Err(ProcessorManagerError::DuplicateProcessor(id))
        } else {
            Ok(())
        }
    }
}

fn ordered_ids(ids: impl IntoIterator<Item = ProcessorId>, policy: ProcessorExecutionPolicy) -> Vec<ProcessorId> {
    let mut ids: Vec<_> = ids.into_iter().collect();
    if policy == ProcessorExecutionPolicy::ReverseInsertionOrder {
        ids.reverse();
    }
    ids
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProcessorManagerError {
    #[error("processor `{0:?}` is already owned by this manager")]
    DuplicateProcessor(ProcessorId),
    #[error("processor group `{0:?}` is already owned by this manager")]
    DuplicateGroup(ProcessorGroupId),
    #[error("processor group `{0:?}` is not owned by this manager")]
    MissingGroup(ProcessorGroupId),
}
