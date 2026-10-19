use super::*;

#[test]
fn native_queue_and_owner_limits_reject_whole_occurrences_without_growing_workers() {
    let _guard = GATE_LOCK.lock().unwrap();
    let (mut engine, source, capture) = fixture();
    let service = worker::Service::with_prepare(gated);
    let lifetimes = service.native_lifetimes();
    let (started, release) = gate();
    let first = owner(service.clone());
    enqueue(&mut engine, &first, source, capture).unwrap();
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 1..16 {
        enqueue(&mut engine, &first, source, capture).unwrap();
    }
    assert!(enqueue(&mut engine, &first, source, capture)
        .unwrap_err()
        .contains("16"));
    let mut owners = vec![first];
    for _ in 0..3 {
        let next = owner(service.clone());
        for _ in 0..16 {
            enqueue(&mut engine, &next, source, capture).unwrap();
        }
        owners.push(next);
    }
    let last = owner(service.clone());
    enqueue(&mut engine, &last, source, capture).unwrap();
    assert!(enqueue(&mut engine, &last, source, capture)
        .unwrap_err()
        .contains("queue is full"));
    assert_eq!(service.admission.metrics().active, 65);
    assert_eq!(lifetimes.metrics().active, 1);
    owners.push(last);
    release.send(()).unwrap();
    wait(|| owners.iter().all(ready));
    drop(owners);
    drop(service);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn global_ticket_capacity_precedes_snapshot_or_worker_admission() {
    let (mut engine, source, capture) = fixture();
    let service = worker::Service::with_prepare(worker::Job::run);
    let permits = (0..256)
        .map(|_| service.admission.try_reserve().unwrap())
        .collect::<Vec<_>>();
    let owner = owner(service.clone());
    assert!(enqueue(&mut engine, &owner, source, capture)
        .unwrap_err()
        .contains("admission is full"));
    assert!(owner.lock().unwrap().pending.is_empty());
    assert!(owner.lock().unwrap().wake.is_none());
    assert_eq!(service.native_lifetimes().metrics().active, 0);
    drop(permits);
    assert_eq!(service.admission.metrics().active, 0);
}

fn panic_after_gate(job: &worker::Job) -> Result<RetainedPayload<PreparedCommand>, String> {
    let gate = GATE.lock().unwrap().take();
    if let Some((started, wait)) = gate {
        started.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        panic!("injected preparation failure");
    }
    job.run()
}
#[test]
fn producer_failure_resolves_every_accepted_ticket_and_the_next_admission_recovers() {
    let _guard = GATE_LOCK.lock().unwrap();
    let (mut engine, source, capture) = fixture();
    let service = worker::Service::with_prepare(panic_after_gate);
    let lifetimes = service.native_lifetimes();
    let owner = owner(service.clone());
    let (started, release) = gate();
    enqueue(&mut engine, &owner, source, capture).unwrap();
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..3 {
        enqueue(&mut engine, &owner, source, capture).unwrap();
    }
    release.send(()).unwrap();
    wait(|| ready(&owner));
    assert!(owner
        .lock()
        .unwrap()
        .pending
        .iter()
        .all(|delivery| delivery.ticket.0.result.get().unwrap().is_err()));
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
    enqueue(&mut engine, &owner, source, capture).unwrap();
    wait(|| ready(&owner));
    assert!(owner
        .lock()
        .unwrap()
        .pending
        .back()
        .unwrap()
        .ticket
        .0
        .result
        .get()
        .unwrap()
        .is_ok());
    drop(owner);
    drop(service);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
}
