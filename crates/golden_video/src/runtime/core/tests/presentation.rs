use super::*;

struct PresentBackend {
    inner: NullBackend,
    pending: Rc<std::cell::Cell<bool>>,
}

impl Backend for PresentBackend {
    type Resources = <NullBackend as Backend>::Resources;
    fn initialize(&mut self) -> Result<(), BackendError> {
        self.inner.initialize()
    }
    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError> {
        self.inner.resource_bytes(plan)
    }
    fn prepare(&mut self, plan: &CompiledPlan, generation: GenerationInfo) -> Result<Self::Resources, BackendError> {
        self.inner.prepare(plan, generation)
    }
    fn submit(
        &mut self,
        resources: &Self::Resources,
        plan: &CompiledPlan,
        token: CompletionToken,
        time: TimeSnapshot,
    ) -> Result<(), BackendError> {
        self.inner.submit(resources, plan, token, time)
    }
    fn presentation_pending(&self, _: &Self::Resources) -> bool {
        self.pending.get()
    }
    fn present(
        &mut self,
        resources: &Self::Resources,
        plan: &CompiledPlan,
        token: CompletionToken,
        time: TimeSnapshot,
    ) -> Result<(), BackendError> {
        self.inner.submit(resources, plan, token, time)?;
        self.pending.set(false);
        Ok(())
    }
    fn completion(&mut self) -> Result<CompletionToken, BackendError> {
        self.inner.completion()
    }
    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError> {
        self.inner.wait_for_completion(timeout)
    }
    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError> {
        self.inner.reset(device)
    }
}

struct LossOnExit(NullControl);
impl Drop for LossOnExit {
    fn drop(&mut self) {
        self.0.lose_device();
    }
}

#[test]
fn presentation_uses_initialized_content_and_keeps_full_completion_and_retirement_ownership() {
    let runtime = Uuid::new_v4();
    let device = DeviceGeneration::new(runtime, 0);
    let control = NullControl::new();
    let pending = Rc::new(std::cell::Cell::new(true));
    let backend = PresentBackend {
        inner: NullBackend::new(
            NullConfig {
                auto_complete: false,
                ..Default::default()
            },
            device,
            control.clone(),
        )
        .unwrap(),
        pending: pending.clone(),
    };
    let mut core = RenderCore::admit(
        backend,
        device,
        ResourceLimits {
            max_bytes: 64,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
    )
    .unwrap();
    let _loss = LossOnExit(control.clone());
    core.initialize().unwrap();
    let identity = PlanIdentity {
        project: ProjectEpoch::new(runtime, 0),
        revision: PlanRevision::new(1),
    };
    let prepared = core.prepare(identity, plan(1)).unwrap();
    core.install(prepared).unwrap();
    assert_eq!(
        core.submit_presentation(time()),
        Ok(None),
        "cached content must first be initialized by a content frame"
    );
    let first = core.submit(time()).unwrap().unwrap().token;
    let redraw = core.submit_presentation(time()).unwrap().unwrap().token;
    assert!(redraw.serial > first.serial);
    assert_eq!(core.snapshot().in_flight, 2);
    pending.set(true);
    assert_eq!(core.submit_presentation(time()), Err(CoreError::InFlightFull));
    control.complete_through(first);
    core.poll_completion().unwrap();
    control.fail_next_submit();
    let before = core.snapshot();
    assert!(core.submit_presentation(time()).is_err());
    assert_eq!(
        core.snapshot().last_submitted,
        before.last_submitted,
        "pre-use failure rolls back its claim"
    );
    assert!(pending.get());
    core.clear_live().unwrap();
    assert_eq!(core.snapshot().retiring_generations, 1);
    assert_eq!(
        control.snapshot().live,
        1,
        "the presentation use retains its entire resource generation"
    );
    control.complete_through(redraw);
    core.poll_completion().unwrap();
    assert_eq!(core.snapshot().retiring_generations, 0);
    assert_eq!(control.snapshot().live, 0);
}
