use super::*;

impl<T: Node> ProductionState<T> {
    /// Resolve once, then admit compilation only for a new authoritative schedule.
    pub(crate) fn request_schedule_compilation(&mut self, affected: &str) {
        if let Err(error) = self.engine.resolve_if_needed() {
            self.last_runtime_plane_error = Some(error.to_string());
            return;
        }
        if self.requested_schedule_revision != self.engine.schedule_revision() {
            self.request_compilation(affected);
        }
    }

    pub(crate) fn request_compilation(&mut self, affected: &str) {
        if let Err(error) = self.request_compilation_inner(affected) {
            self.last_runtime_plane_error = Some(error);
        }
    }

    pub(crate) fn recompile_blocking(&mut self, affected: &str) -> Result<(), String> {
        let ticket = self.request_compilation_inner(affected)?;
        loop {
            let completion = self.compiler.complete().map_err(|error| error.to_string())?;
            let completed_ticket = completion.ticket;
            self.apply_completion(completion);
            if completed_ticket == ticket {
                return self.last_runtime_plane_error.take().map_or(Ok(()), Err);
            }
        }
    }

    fn request_compilation_inner(&mut self, affected: &str) -> Result<u64, String> {
        if self.engine.is_resolve_pending() {
            self.engine
                .resolve()
                .map_err(|error| format!("failed to resolve runtime schedule before compilation: {error}"))?;
        }
        self.compilation_revision.0 = self.compilation_revision.0.saturating_add(1);
        let snapshot = Arc::new(EngineCompileSnapshot::capture(&self.engine));
        let mut changes = RuntimeChangeSet::new();
        changes.mark(affected);
        let admission = self
            .compiler
            .handle()
            .request(CompileRequest {
                project: snapshot.clone(),
                revision: self.compilation_revision,
                changes,
                previous: Some(self.semantic.current_generation()),
            })
            .map_err(|error| error.to_string())?;
        if let Some(superseded_ticket) = admission.superseded_ticket {
            self.pending_compilations.remove(&superseded_ticket);
        }
        let ticket = admission.ticket;
        self.requested_schedule_revision = snapshot.schedule_revision;
        self.pending_compilations.insert(
            ticket,
            PendingCompilation {
                snapshot,
                project_generation: self.project_generation,
            },
        );
        Ok(ticket)
    }

    pub(super) fn apply_completed_compilations(&mut self) {
        loop {
            match self.compiler.try_complete() {
                Ok(Some(completion)) => self.apply_completion(completion),
                Ok(None) => break,
                Err(error) => {
                    self.last_runtime_plane_error = Some(error.to_string());
                    break;
                }
            }
        }
    }

    pub(super) fn apply_completion(&mut self, completion: CompilationCompletion) {
        let Some(pending) = self.pending_compilations.remove(&completion.ticket) else {
            return;
        };
        if pending.project_generation != self.project_generation {
            return;
        }
        if completion.revision != self.compilation_revision
            || self.engine.is_resolve_pending()
            || pending.snapshot.schedule_revision != self.engine.schedule_revision()
        {
            return;
        }
        let generation = match completion.result {
            Ok(generation) => generation,
            Err(error) => {
                self.last_runtime_plane_error = Some(error.to_string());
                return;
            }
        };
        if let Err(error) = self.install_generation(generation, &pending.snapshot) {
            self.last_runtime_plane_error = Some(error);
        }
    }

    fn install_generation(
        &mut self,
        generation: Arc<RuntimeGeneration>,
        snapshot: &EngineCompileSnapshot,
    ) -> Result<(), String> {
        let layout = snapshot.compiled_layout();
        let (next_mailbox, next_input_generation) = make_input_generation(snapshot, false)?;
        let work_count = generation.schedule.work_count();
        next_input_generation.activate()?;
        self.input_generation.deactivate()?;
        self.input_plane.current.store(next_input_generation.clone());
        self.apply_dense_inputs();
        self.semantic.swap_generation(generation);
        self.compiled_schedule_revision = snapshot.schedule_revision;
        seed_semantic_inputs(&mut self.semantic, snapshot)?;
        self.input_mailbox = next_mailbox;
        self.input_generation = next_input_generation;
        self.slot_to_node = layout.slot_to_node();
        self.scheduled_node_to_work = layout.scheduled_node_to_work();
        self.scheduled_work_nodes = layout.scheduled_nodes.iter().map(|scheduled| scheduled.node).collect();
        self.input_work_count = layout.parameters.len();
        self.dirty = DirtySet::new(work_count);
        self.selected_work.clear();
        self.selected_work.reserve(work_count);
        self.work_selector.prepare(work_count);
        Ok(())
    }

    pub(crate) fn compile_project_candidate(
        &mut self,
        engine: &mut Engine<T>,
    ) -> Result<CompiledProjectCandidate, String> {
        engine
            .resolve_if_needed()
            .map_err(|error| format!("failed to resolve project candidate: {error}"))?;
        let snapshot = Arc::new(EngineCompileSnapshot::capture(engine));
        let mut changes = RuntimeChangeSet::new();
        changes.mark("project.replace");
        let admission = self
            .compiler
            .handle()
            .request(CompileRequest {
                project: snapshot.clone(),
                revision: ProjectRevision(1),
                changes,
                previous: None,
            })
            .map_err(|error| error.to_string())?;
        if let Some(superseded_ticket) = admission.superseded_ticket {
            self.pending_compilations.remove(&superseded_ticket);
        }
        let ticket = admission.ticket;

        loop {
            let completion = self.compiler.complete().map_err(|error| error.to_string())?;
            if completion.ticket == ticket {
                let generation = completion.result.map_err(|error| error.to_string())?;
                return Ok(CompiledProjectCandidate { snapshot, generation });
            }
            self.apply_completion(completion);
        }
    }
}
