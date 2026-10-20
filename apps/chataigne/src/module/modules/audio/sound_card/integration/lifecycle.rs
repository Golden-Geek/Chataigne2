use super::*;

impl SoundCardModule {
    pub(in super::super) fn runtime_matches_requested_sample_rate(&self) -> bool {
        let Ok(sample_rate) = self.requested_sample_rate() else {
            return false;
        };
        let Ok(driver) = self.requested_driver() else {
            return false;
        };
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.sample_rate() == sample_rate && runtime.driver() == driver.as_ref())
        {
            return true;
        }
        false
    }

    pub(in super::super) fn drive_runtime(&mut self, ctx: &mut ProcessCtx) {
        self.poll_runtime_worker(ctx);
        if self.requested_driver().ok() == Some(None) {
            self.retire_unselected_runtime(ctx);
            self.poll_runtime(ctx);
            return;
        }
        if !self.node_data().effective_enabled {
            if let Some(runtime) = self.runtime.as_mut() {
                let _ = runtime.set_enabled(false);
            }
            self.poll_runtime(ctx);
            return;
        }
        if self.configuration_dirty {
            if let Some(snapshot) = ctx.tree_snapshot_arc() {
                self.synchronize_derived_structure(ctx, snapshot.as_ref());
            }
        }
        self.request_runtime_start(ctx);
        self.poll_runtime(ctx);

        if !self.configuration_dirty {
            return;
        }
        if self.runtime_matches_requested_sample_rate() {
            if let Some(snapshot) = ctx.tree_snapshot_arc() {
                self.refresh_configuration(ctx, snapshot.as_ref());
                if self.configuration_dirty && !self.runtime_matches_requested_sample_rate() {
                    self.request_runtime_start(ctx);
                }
            } else if let Some(wake) = &self.runtime_wake {
                wake.wake();
            }
        } else {
            self.request_runtime_start(ctx);
        }
    }

    pub(in super::super) fn request_runtime_start(&mut self, ctx: &mut ProcessCtx) {
        if !self.node_data().effective_enabled {
            return;
        }
        let sample_rate = match self.requested_sample_rate() {
            Ok(sample_rate) => sample_rate,
            Err(error) => {
                self.set_runtime_error(ctx, self.id(), error.as_str());
                return;
            }
        };
        let driver = match self.requested_driver() {
            Ok(driver) => driver,
            Err(error) => {
                self.set_runtime_error(ctx, self.id(), error.as_str());
                return;
            }
        };
        if driver.is_none() {
            self.retire_unselected_runtime(ctx);
            return;
        }
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.sample_rate() == sample_rate && runtime.driver() == driver.as_ref())
        {
            self.runtime_request = None;
            self.runtime_retry_at = None;
            self.runtime_retry.reset();
            return;
        }
        if self
            .runtime_request
            .as_ref()
            .is_some_and(|request| request.sample_rate() == sample_rate && request.driver() == driver.as_ref())
        {
            return;
        }
        if let Some((retry_sample_rate, retry_driver, retry_at)) = self.runtime_retry_at.as_ref() {
            if *retry_sample_rate != sample_rate || retry_driver != &driver {
                self.runtime_retry_at = None;
                self.runtime_retry.reset();
            } else if Instant::now() < *retry_at {
                return;
            } else {
                self.runtime_retry_at = None;
            }
        }
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.sample_rate() != sample_rate || runtime.driver() != driver.as_ref())
        {
            self.runtime = None;
        }
        if self.runtime_worker.is_none() {
            let Some(wake) = self.runtime_wake.clone() else {
                self.set_runtime_error(
                    ctx,
                    self.id(),
                    "Sound Card runtime cannot start without an engine wake sender",
                );
                return;
            };
            match runtime::SoundCardRuntimeWorker::spawn(wake) {
                Ok(worker) => self.runtime_worker = Some(worker),
                Err(error) => {
                    self.schedule_runtime_retry(sample_rate, driver.clone());
                    self.set_runtime_error(ctx, self.id(), error.as_str());
                    return;
                }
            }
        }
        let request = self
            .runtime_worker
            .as_mut()
            .expect("Sound Card runtime worker was initialized");
        request.set_enabled(true);
        let request = request.request_start_for_driver(sample_rate, driver.clone());
        match request {
            Ok(request) => self.runtime_request = Some(request),
            Err(error) => {
                if matches!(error, runtime::StartRequestError::Disconnected) {
                    self.runtime_worker = None;
                }
                self.schedule_runtime_retry(sample_rate, driver);
                self.set_runtime_error(ctx, self.id(), &error.to_string());
            }
        }
    }

    pub(in super::super) fn poll_runtime_worker(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..2 {
            let poll = self.runtime_worker.as_ref().map(runtime::SoundCardRuntimeWorker::poll);
            match poll {
                None | Some(runtime::SoundCardRuntimeWorkerPoll::Pending) => return,
                Some(runtime::SoundCardRuntimeWorkerPoll::Started(started)) => {
                    self.handle_runtime_started(ctx, started);
                }
                Some(runtime::SoundCardRuntimeWorkerPoll::Disconnected) => {
                    self.runtime_worker = None;
                    if self.requested_driver().ok() == Some(None) {
                        self.runtime_request = None;
                        self.runtime_retry_at = None;
                        self.runtime_retry.reset();
                        return;
                    }
                    let retry_sample_rate = self
                        .runtime_request
                        .take()
                        .map(|request| (request.sample_rate(), request.driver().cloned()))
                        .or_else(|| Some((self.requested_sample_rate().ok()?, self.requested_driver().ok()?)));
                    if let Some((sample_rate, driver)) = retry_sample_rate {
                        self.schedule_runtime_retry(sample_rate, driver);
                    }
                    self.set_runtime_error(ctx, self.id(), "Sound Card runtime worker stopped unexpectedly");
                    return;
                }
            }
        }
        if self
            .runtime_worker
            .as_ref()
            .is_some_and(runtime::SoundCardRuntimeWorker::has_pending)
        {
            if let Some(wake) = &self.runtime_wake {
                wake.wake();
            }
        }
    }

    fn retire_unselected_runtime(&mut self, ctx: &mut ProcessCtx) {
        self.runtime = None;
        self.configuration_dirty = false;
        self.clear_device_connection_warnings(ctx);
        self.clear_runtime_error(ctx);
        let Some(worker) = self.runtime_worker.as_mut() else {
            self.runtime_request = None;
            self.runtime_retry_at = None;
            self.runtime_retry.reset();
            return;
        };
        worker.set_enabled(false);
        if self
            .runtime_request
            .as_ref()
            .is_some_and(|request| request.driver().is_none())
        {
            return;
        }
        // The startup gate was cancelled even if idle admission is full. A later
        // reselection must not reuse that cancelled in-flight identity.
        self.runtime_request = None;
        // Keep the same admitted lifecycle until its queued/in-flight startup
        // and last native cleanup finish. Dropping and respawning would lose that fence.
        let sample_rate = golden_audio::SampleRate::default();
        match worker.request_start_for_driver(sample_rate, None) {
            Ok(request) => {
                self.runtime_request = Some(request);
                self.runtime_retry_at = None;
                self.runtime_retry.reset();
            }
            Err(runtime::StartRequestError::Full) => self.schedule_runtime_retry(sample_rate, None),
            Err(runtime::StartRequestError::Disconnected) => {
                self.runtime_worker = None;
                self.runtime_request = None;
            }
            Err(error) => self.set_runtime_error(ctx, self.id(), &error.to_string()),
        }
    }

    fn handle_runtime_started(&mut self, ctx: &mut ProcessCtx, started: runtime::SoundCardRuntimeStarted) {
        if !self.node_data().effective_enabled {
            self.runtime_request = None;
            self.configuration_dirty = true;
            drop(started);
            return;
        }
        if self.runtime_request.as_ref() != Some(&started.request) {
            drop(started);
            return;
        }
        self.runtime_request = None;
        if self.requested_sample_rate().ok() != Some(started.request.sample_rate())
            || self.requested_driver().ok() != Some(started.request.driver().cloned())
        {
            drop(started);
            return;
        }
        match started.result {
            Ok(runtime) => {
                let mut runtime = *runtime;
                let Some(wake) = self.runtime_wake.clone() else {
                    drop(runtime);
                    self.set_runtime_error(ctx, self.id(), "Sound Card runtime lost its engine wake sender");
                    return;
                };
                if let Err(error) = runtime.enable_notifications(wake) {
                    drop(runtime);
                    self.schedule_runtime_retry(started.request.sample_rate(), started.request.driver().cloned());
                    self.set_runtime_error(ctx, self.id(), error.as_str());
                    return;
                }
                self.clear_device_connection_warnings(ctx);
                self.runtime = Some(runtime);
                self.runtime_retry.reset();
                self.runtime_retry_at = None;
                self.configuration_dirty = true;
            }
            Err(error) => {
                self.schedule_runtime_retry(started.request.sample_rate(), started.request.driver().cloned());
                self.set_runtime_error(ctx, self.id(), error.as_str());
            }
        }
    }

    pub(super) fn schedule_runtime_retry(
        &mut self,
        sample_rate: golden_audio::SampleRate,
        driver: Option<golden_audio::BackendId>,
    ) {
        let now = Instant::now();
        let retry_at = self.runtime_retry.schedule(now);
        self.runtime_retry_at = Some((sample_rate, driver, retry_at));
        if let Some(wake) = &self.runtime_wake {
            wake.wake_after(retry_at.saturating_duration_since(now));
        }
    }
}
