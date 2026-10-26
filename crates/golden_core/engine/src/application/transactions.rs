use super::*;
use golden_runtime::PendingControl;

type ActorTransaction = (
    ProjectGeneration,
    UiAck,
    UiEventBatch,
    bool,
    ApplicationTransactionTiming,
);

/// An actor-admitted batch. No engine acknowledgement exists until completion.
pub struct PendingUiTransactionBatch {
    pending: PendingControl<Result<Vec<ActorTransaction>, ApplicationError>>,
}

impl PendingUiTransactionBatch {
    /// Waits for the authoritative batch result or a typed completion failure.
    pub fn wait(self) -> Result<AppliedUiTransactionBatch, ApplicationError> {
        let receipt = self.pending.wait()?;
        let lock_wait = receipt.queue_wait;
        let pending = receipt.output?;

        let transactions = pending
            .into_iter()
            .enumerate()
            .map(
                |(index, (project_generation, acknowledgement, events, preferences_changed, mut timing))| {
                    if index == 0 {
                        timing.lock_wait = lock_wait;
                    }
                    timing.total = timing
                        .total
                        .saturating_add(if index == 0 { lock_wait } else { Duration::ZERO });
                    AppliedUiTransaction {
                        project_generation,
                        acknowledgement,
                        events,
                        preferences_changed,
                        timing,
                    }
                },
            )
            .collect::<Vec<_>>();
        let preferences_changed = transactions.iter().any(|transaction| transaction.preferences_changed);

        Ok(AppliedUiTransactionBatch {
            transactions,
            preferences_changed,
            lock_wait,
        })
    }
}

impl<T: ProjectLifecycle> ProductionRuntime<T> {
    /// Admits and waits for an ordered batch; admission/completion failures remain typed.
    pub fn apply_ui_transaction_batch(
        &self,
        intents: Vec<UiEditIntent>,
        ui_client_instance_id: Option<&str>,
        stop_after_failure: bool,
    ) -> Result<AppliedUiTransactionBatch, ApplicationError> {
        self.submit_ui_transaction_batch(intents, ui_client_instance_id, stop_after_failure)?
            .wait()
    }

    /// Applies one UI transaction and publishes its captured observation delta.
    pub fn apply_ui_transaction(
        &self,
        intent: UiEditIntent,
        ui_client_instance_id: Option<&str>,
    ) -> Result<AppliedUiTransaction, ApplicationError> {
        Ok(self
            .apply_ui_transaction_batch(vec![intent], ui_client_instance_id, true)?
            .transactions
            .into_iter()
            .next()
            .expect("single transaction batch always returns one result"))
    }

    /// Applies ordered UI transactions under one control-plane lock.
    ///
    /// When `stop_after_failure` is true, later transactions receive deterministic skipped
    /// acknowledgements and are not applied.
    pub fn submit_ui_transaction_batch(
        &self,
        intents: Vec<UiEditIntent>,
        ui_client_instance_id: Option<&str>,
        stop_after_failure: bool,
    ) -> Result<PendingUiTransactionBatch, ApplicationError> {
        self.submit_ui_transactions(intents, ui_client_instance_id, None, stop_after_failure)
    }

    /// Admits requests scoped to a live connection. Closed queued requests never execute.
    pub fn submit_ui_client_transaction_batch(
        &self,
        intents: Vec<UiEditIntent>,
        client: &UiClientLease,
        stop_after_failure: bool,
    ) -> Result<PendingUiTransactionBatch, ApplicationError> {
        self.submit_ui_transactions(
            intents,
            client.client_instance_id(),
            Some(client.clone()),
            stop_after_failure,
        )
    }

    fn submit_ui_transactions(
        &self,
        intents: Vec<UiEditIntent>,
        ui_client_instance_id: Option<&str>,
        client: Option<UiClientLease>,
        stop_after_failure: bool,
    ) -> Result<PendingUiTransactionBatch, ApplicationError> {
        let ui_client_instance_id = ui_client_instance_id.map(str::to_owned);
        let read_model = self.inner.read_model.clone();
        let project_document = self.inner.project_document.clone();
        let publication_hook = self.inner.read_model_publication_hook.clone();
        #[cfg(test)]
        let preferences_subtree_collection_count = self.inner.preferences_subtree_collection_count.clone();
        let pending = self
            .inner
            .control.handle()
            .submit(move |state| {
                let project_generation = read_model.current_project_generation();
                if state.release_closed_ui_session() || state.release_rebound_ui_session(client.as_ref()) {
                    let before = state.engine.ui_event_log().last().map(|event| event.time);
                    let capture = read_model.collect_event_batch(&mut state.engine, before);
                    publish_event_capture(&project_document, &read_model, &publication_hook, &mut state.engine, capture);
                }
                if client.as_ref().is_some_and(UiClientLease::is_closed) {
                    return Err(ApplicationError::Control(ControlError::cancelled()));
                }
                if let Some(error) = state.project_pause_error().map(str::to_string) {
                    return Ok(intents
                        .into_iter()
                        .map(|_| {
                            let before = state.engine.ui_event_log().last().map(|event| event.time);
                            let capture = read_model.collect_event_batch(&mut state.engine, before);
                            let events = publish_event_capture(
                                &project_document,
                                &read_model,
                                &publication_hook,
                                &mut state.engine,
                                capture,
                            );
                            (
                                project_generation,
                                rejected_ack(
                                    &state.engine,
                                    "project_runtime_paused",
                                    format!("project runtime is paused: {error}"),
                                ),
                                events,
                                false,
                                ApplicationTransactionTiming::default(),
                            )
                        })
                        .collect());
                }
                let mut failed = false;
                let mut opened_edit_session: Option<String> = None;
                let mut pending = Vec::with_capacity(intents.len());
                #[cfg(test)]
                preferences_subtree_collection_count.fetch_add(1, Ordering::Relaxed);
                let mut preferences_nodes = preferences_subtree_node_ids(&state.engine);

                for intent in intents {
                    let intent_started = Instant::now();
                    let is_matching_end_edit = opened_edit_session.as_ref().is_some_and(
                        |active_id| matches!(&intent, UiEditIntent::EndEdit { client_edit_id } if client_edit_id == active_id),
                    );
                    if failed && stop_after_failure && !is_matching_end_edit {
                        let cursor = state.engine.ui_event_log().last().map(|event| event.time);
                        let capture = read_model.collect_event_batch(&mut state.engine, cursor);
                        let events = publish_event_capture(
                            &project_document,
                            &read_model,
                            &publication_hook,
                            &mut state.engine,
                            capture,
                        );
                        pending.push((
                            project_generation,
                            skipped_after_failed_batch_ack(&state.engine),
                            events,
                            false,
                            ApplicationTransactionTiming {
                                lock_wait: Duration::ZERO,
                                total: intent_started.elapsed(),
                                ..Default::default()
                            },
                        ));
                        continue;
                    }

                    let begin_edit_id = match &intent {
                        UiEditIntent::BeginEdit { client_edit_id, .. } => Some(client_edit_id.clone()),
                        _ => None,
                    };
                    let end_edit_id = match &intent {
                        UiEditIntent::EndEdit { client_edit_id } => Some(client_edit_id.clone()),
                        _ => None,
                    };

                    let before_event_time = state.engine.ui_event_log().last().map(|event| event.time);
                    let is_app_operation = matches!(&intent, UiEditIntent::InvokeAppOperation { .. });
                    let apply_started = Instant::now();
                    let mut acknowledgement =
                        apply_ui_intent_to_engine(&mut state.engine, intent, ui_client_instance_id.as_deref());
                    let began_edit_session = acknowledgement.success && begin_edit_id.is_some();
                    if acknowledgement.success && !is_app_operation
                        && let Err(error) = T::settle_pending_operations(&mut state.engine)
                    {
                        acknowledgement = rejected_ack(
                            &state.engine,
                            "app_operation_failed",
                            error,
                        );
                    }
                    if let Err(error) = service_script_app_operations(&mut state.engine) {
                        eprintln!("[golden] script app-operation callback delivery failed: {error}");
                    }
                    let apply = apply_started.elapsed();
                    let event_collect_started = Instant::now();
                    let capture = read_model.collect_event_batch(&mut state.engine, before_event_time);
                    let preferences_changed = if event_batch_has_structural_graph_changes(capture.batch()) {
                        #[cfg(test)]
                        preferences_subtree_collection_count.fetch_add(1, Ordering::Relaxed);
                        let updated_preferences_nodes = preferences_subtree_node_ids(&state.engine);
                        let changed = event_batch_changes_preferences(capture.batch(), &preferences_nodes)
                            || event_batch_changes_preferences(capture.batch(), &updated_preferences_nodes);
                        preferences_nodes = updated_preferences_nodes;
                        changed
                    } else {
                        event_batch_changes_preferences(capture.batch(), &preferences_nodes)
                    };
                    let events = publish_event_capture(
                        &project_document,
                        &read_model,
                        &publication_hook,
                        &mut state.engine,
                        capture,
                    );
                    let event_collect = event_collect_started.elapsed();
                    state.track_ui_session_lease(client.as_ref(), began_edit_session);
                    if acknowledgement.success {
                        if let Some(client_edit_id) = begin_edit_id {
                            opened_edit_session = Some(client_edit_id);
                        }
                        if end_edit_id.as_ref() == opened_edit_session.as_ref() {
                            opened_edit_session = None;
                        }
                    }
                    failed |= !acknowledgement.success;
                    pending.push((
                        project_generation,
                        acknowledgement,
                        events,
                        preferences_changed,
                        ApplicationTransactionTiming {
                            lock_wait: Duration::ZERO,
                            apply,
                            event_collect,
                            total: intent_started.elapsed(),
                        },
                    ));
                }
                state.request_schedule_compilation("ui.schedule");
                Ok(pending)
            })
            ?;
        Ok(PendingUiTransactionBatch { pending })
    }
}
