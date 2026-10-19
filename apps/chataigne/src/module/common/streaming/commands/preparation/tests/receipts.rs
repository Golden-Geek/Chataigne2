use super::*;

#[test]
fn discarded_publication_retries_one_occurrence_then_releases_its_wake_slot() {
    let (mut engine, source, capture) = fixture();
    let service = worker::Service::with_prepare(worker::Job::run);
    let lifetimes = service.native_lifetimes();
    let owner = owner(service.clone());
    enqueue(&mut engine, &owner, source, capture).unwrap();
    wait(|| ready(&owner));
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    owner.lock().unwrap().service(&mut ctx);
    let first = match &ctx.edits.pending[0].edit {
        Edit::EmitCustomEvent { event } => event
            .local_payload::<RetainedPayload<PreparedCommand>>()
            .unwrap()
            .clone(),
        _ => panic!("native publication"),
    };
    drop(ctx);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    owner.lock().unwrap().service(&mut ctx);
    let retry = match &ctx.edits.pending[0].edit {
        Edit::EmitCustomEvent { event } => event.local_payload::<RetainedPayload<PreparedCommand>>().unwrap(),
        _ => panic!("retry"),
    };
    assert!(std::ptr::eq(&*first, &**retry));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    owner
        .lock()
        .unwrap()
        .service(&mut ProcessCtx::new(ExecutionPhase::EngineTick, engine.time));
    assert!(owner.lock().unwrap().pending.is_empty());
    assert!(owner.lock().unwrap().wake.is_none());
    assert!(first.is_live());
    owner.lock().unwrap().clear();
    assert!(!first.is_live(), "destroy cancels even already-published envelopes");
    drop(owner);
    drop(service);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn destroy_does_not_join_a_blocked_job_or_replay_it_into_a_restored_source() {
    let _guard = GATE_LOCK.lock().unwrap();
    let (mut engine, source, capture) = fixture();
    let service = worker::Service::with_prepare(gated);
    let lifetimes = service.native_lifetimes();
    let owner = owner(service.clone());
    let (started, release) = gate();
    enqueue(&mut engine, &owner, source, capture).unwrap();
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    let ticket = owner.lock().unwrap().pending.front().unwrap().ticket.clone();
    let old = owner.lock().unwrap().wake.as_ref().unwrap().0.clone();
    let begin = Instant::now();
    owner.lock().unwrap().clear();
    assert!(begin.elapsed() < Duration::from_millis(100));
    engine.edits.push(Edit::RemoveNode { node: source });
    engine.apply_edits().unwrap();
    assert!(old.is_closed());
    assert!(engine.undo().unwrap());
    release.send(()).unwrap();
    wait(|| ticket.0.result.get().is_some());
    assert!(ticket.0.result.get().unwrap().is_err());
    enqueue(&mut engine, &owner, source, capture).unwrap();
    wait(|| ready(&owner));
    assert!(owner
        .lock()
        .unwrap()
        .pending
        .front()
        .unwrap()
        .ticket
        .0
        .result
        .get()
        .unwrap()
        .is_ok());
    assert_ne!(old, owner.lock().unwrap().wake.as_ref().unwrap().0);
    drop(owner);
    drop(service);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
}
