use super::*;

fn id() -> ApplicationId {
    ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4()))
}
fn key(argument: &str) -> Arc<ApplicationRuntimeKey> {
    Arc::new(
        ApplicationRuntimeKey::new(&ApplicationDefinition {
            executable: Some(std::env::current_exe().unwrap().to_string_lossy().into()),
            arguments: vec![argument.into()],
            ..Default::default()
        })
        .unwrap(),
    )
}
fn target(key: Option<Arc<ApplicationRuntimeKey>>, source: &golden_io::RevisionSource) -> Target {
    let reader = source.reader();
    Target {
        definition: key,
        retain_existing: false,
        guard: Some(reader.guard_at(reader.current())),
        admission: Arc::from([]),
        definition_revision: Some(1),
        definition_instance: Some(uuid::Uuid::new_v4()),
    }
}
fn available(request: &Request) -> impl Iterator<Item = (ApplicationRuntimeId, OwnerKey)> + '_ {
    request.owners.iter().map(|(id, owner)| (*id, owner.key.clone()))
}

#[test]
fn thousands_of_identical_consumers_use_one_immutable_owner_and_compact_callback_status() {
    let source = golden_io::RevisionSource::default();
    let key = key("shared");
    let targets = (0..MAX_CONSUMERS)
        .map(|_| (id(), target(Some(key.clone()), &source)))
        .collect();
    let grouped = request(targets, &Request::default(), std::iter::empty());
    assert_eq!(grouped.owners.len(), 1);
    assert_eq!(grouped.bindings.len(), MAX_CONSUMERS);
    let owner = grouped.owners.values().next().unwrap();
    assert!(owner.admitted(&grouped.targets));
    assert_eq!(owner.observations.len(), MAX_CONSUMERS);
    let process = agent::Agent::new(*grouped.owners.keys().next().unwrap(), agent::Native).status;
    assert!(serde_json::to_vec(&process).unwrap().len() < 1024);
    assert!(
        !serde_json::to_value(process)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("consumers")
    );
}

#[test]
fn simultaneous_consumer_handoff_preserves_runtime_id_and_setting_changes_split_only_the_affected_lease() {
    let source = golden_io::RevisionSource::default();
    let (first, second, third) = (id(), id(), id());
    let shared = key("shared");
    let initial = request(
        [(first, target(Some(shared.clone()), &source))].into(),
        &Request::default(),
        std::iter::empty(),
    );
    let runtime = initial.bindings[&first];
    let handoff = request(
        [
            (second, target(Some(shared.clone()), &source)),
            (third, target(Some(shared), &source)),
        ]
        .into(),
        &initial,
        available(&initial),
    );
    assert_eq!(handoff.bindings[&second], runtime);
    assert_eq!(handoff.bindings[&third], runtime);
    let changed = request(
        [
            (second, target(Some(key("new")), &source)),
            (third, handoff.targets[&third].clone()),
        ]
        .into(),
        &handoff,
        available(&handoff),
    );
    assert_ne!(changed.bindings[&second], runtime);
    assert_eq!(changed.bindings[&third], runtime);
    assert_eq!(changed.owners.len(), 2);
}

#[test]
fn one_invalid_consumer_cannot_cancel_another_current_lease_and_no_stale_lease_can_authorize_launch() {
    let a = golden_io::RevisionSource::default();
    let b = golden_io::RevisionSource::default();
    let key = key("shared");
    let grouped = request(
        [(id(), target(Some(key.clone()), &a)), (id(), target(Some(key), &b))].into(),
        &Request::default(),
        std::iter::empty(),
    );
    let owner = grouped.owners.values().next().unwrap();
    a.advance();
    assert!(owner.admitted(&grouped.targets));
    b.advance();
    assert!(!owner.admitted(&grouped.targets));
}

#[test]
fn retained_missing_consumer_keeps_its_previous_owner_but_cannot_authorize_creation() {
    let source = golden_io::RevisionSource::default();
    let consumer = id();
    let initial = request(
        [(consumer, target(Some(key("shared")), &source))].into(),
        &Request::default(),
        std::iter::empty(),
    );
    let mut missing = target(None, &source);
    missing.guard = None;
    missing.retain_existing = true;
    let retained = request([(consumer, missing)].into(), &initial, available(&initial));
    assert_eq!(retained.bindings[&consumer], initial.bindings[&consumer]);
    assert!(!retained.owners.values().next().unwrap().admitted(&retained.targets));
}

#[test]
fn changed_outgoing_settings_keep_the_previous_owner_and_each_consumer_observes_its_own_configuration() {
    let source = golden_io::RevisionSource::default();
    let (outgoing, active) = (id(), id());
    let original = key("old");
    let initial = request(
        [(outgoing, target(Some(original.clone()), &source))].into(),
        &Request::default(),
        std::iter::empty(),
    );
    let runtime = initial.bindings[&outgoing];
    let mut changed = target(Some(key("new")), &source);
    changed.retain_existing = true;
    let retained = request(
        [(outgoing, changed.clone()), (active, target(Some(original), &source))].into(),
        &initial,
        available(&initial),
    );
    assert_eq!(retained.owners.len(), 1);
    let owner = &retained.owners[&runtime];
    assert!(owner.admitted(&retained.targets));
    assert_eq!(
        owner
            .observations
            .iter()
            .find(|c| c.application == outgoing)
            .unwrap()
            .configuration,
        ApplicationConfigurationState::Pending
    );
    assert_eq!(
        owner
            .observations
            .iter()
            .find(|c| c.application == active)
            .unwrap()
            .configuration,
        ApplicationConfigurationState::Current
    );
    let only_outgoing = request([(outgoing, changed)].into(), &retained, available(&retained));
    assert_eq!(only_outgoing.bindings[&outgoing], runtime);
    assert!(!only_outgoing.owners[&runtime].admitted(&only_outgoing.targets));
}
