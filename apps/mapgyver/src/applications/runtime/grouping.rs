//! One immutable normalized launch key per worker; authored consumers remain independent leases.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum OwnerKey {
    Configured(Arc<ApplicationRuntimeKey>),
    Unavailable(ApplicationId),
}
#[derive(Clone)]
pub(super) struct OwnerTarget {
    pub key: OwnerKey,
    pub consumers: Arc<[ApplicationId]>,
    pub observations: Arc<[ApplicationRuntimeConsumer]>,
}
impl OwnerTarget {
    pub fn admitted(&self, targets: &HashMap<ApplicationId, Target>) -> bool {
        self.consumers.iter().any(|id| {
            targets.get(id).is_some_and(|target| {
                matches!(&self.key, OwnerKey::Configured(key) if target.definition.as_ref() == Some(key))
                    && target.guard.as_ref().is_some_and(RevisionGuard::is_current)
                    && target.admission.iter().all(RevisionGuard::is_current)
            })
        })
    }
    fn capture_observations(&self, targets: &HashMap<ApplicationId, Target>) -> Arc<[ApplicationRuntimeConsumer]> {
        self.consumers
            .iter()
            .filter_map(|id| {
                targets.get(id).map(|target| ApplicationRuntimeConsumer {
                    application: *id,
                    configuration: match &target.definition {
                        None => ApplicationConfigurationState::Retained,
                        Some(key) if self.key == OwnerKey::Configured(key.clone()) => {
                            ApplicationConfigurationState::Current
                        }
                        Some(_) => ApplicationConfigurationState::Pending,
                    },
                    definition_revision: target.definition_revision,
                    definition_instance: target.definition_instance,
                })
            })
            .collect::<Vec<_>>()
            .into()
    }
}

pub(super) fn request(
    targets: HashMap<ApplicationId, Target>,
    previous: &Request,
    available: impl Iterator<Item = (ApplicationRuntimeId, OwnerKey)>,
) -> Request {
    let mut keys = available.map(|(id, key)| (key, id)).collect::<HashMap<_, _>>();
    let mut grouped = HashMap::<ApplicationRuntimeId, (OwnerKey, Vec<ApplicationId>)>::new();
    let mut bindings = HashMap::new();
    for (id, target) in &targets {
        let previous_key = previous
            .bindings
            .get(id)
            .and_then(|runtime| previous.owners.get(runtime))
            .map(|owner| owner.key.clone());
        let key = match &target.definition {
            Some(_) if target.retain_existing && previous_key.is_some() => previous_key.unwrap(),
            Some(key) => OwnerKey::Configured(key.clone()),
            None => previous_key.unwrap_or(OwnerKey::Unavailable(*id)),
        };
        let runtime = *keys
            .entry(key.clone())
            .or_insert_with(|| ApplicationRuntimeId(uuid::Uuid::new_v4()));
        grouped.entry(runtime).or_insert_with(|| (key, Vec::new())).1.push(*id);
        bindings.insert(*id, runtime);
    }
    let owners = grouped
        .into_iter()
        .map(|(runtime, (key, mut consumers))| {
            consumers.sort_by_key(|id| id.0.0);
            let mut owner = OwnerTarget {
                key,
                consumers: consumers.into(),
                observations: Arc::from([]),
            };
            owner.observations = owner.capture_observations(&targets);
            (runtime, owner)
        })
        .collect();
    Request {
        targets,
        owners,
        bindings,
    }
}

#[cfg(test)]
mod tests;
