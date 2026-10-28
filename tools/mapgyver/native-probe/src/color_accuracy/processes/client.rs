use super::super::protocol::{Report, Request, Response, Role};
use crate::{ProbeResult, process::ReportedProcess, wire};
use mapgyver_native_probe::color_reference::Fixture;
use mapgyver_native_probe::workload::Scenario;
use std::{
    process::Command,
    sync::mpsc::TryRecvError,
    time::{Duration, Instant},
};

pub struct Child {
    process: ReportedProcess<Response>,
    role: Role,
    luid: String,
    finished: bool,
    captured: bool,
    scenario: Scenario,
    fixture: Fixture,
}

impl Child {
    pub fn spawn(
        adapter: u32,
        scenario: Scenario,
        fixture: Fixture,
        role: Role,
        name: &str,
        luid: &str,
    ) -> ProbeResult<Self> {
        let process: ReportedProcess<Response> = ReportedProcess::spawn(
            Command::new(std::env::current_exe()?).args([
                "color-child",
                &adapter.to_string(),
                scenario.name(),
                fixture.name(),
                role.name(),
                name,
            ]),
            wire::line(&Request::Stop)?,
        )?;
        match process.receive(Duration::from_secs(5))? {
            Response::Ready {
                process_id,
                adapter_luid,
                role: reported_role,
                fixture: reported_fixture,
            } if process_id == process.child().id()
                && adapter_luid == luid
                && reported_role == role
                && reported_fixture == fixture => {}
            _ => return Err("invalid accuracy child initialization".into()),
        }
        Ok(Self {
            process,
            role,
            luid: luid.into(),
            finished: false,
            captured: false,
            scenario,
            fixture,
        })
    }
    pub fn capture(&mut self) -> ProbeResult<()> {
        if self.role != Role::Consumer || self.finished || self.captured {
            return Err("invalid accuracy capture request".into());
        }
        self.process.owned.send(&wire::line(&Request::Capture)?)?;
        self.captured = true;
        Ok(())
    }
    fn accept(&mut self, response: Response) -> ProbeResult<Report> {
        let Response::Finished { report } = response else {
            return Err("accuracy child repeated initialization".into());
        };
        if self.finished || (self.role == Role::Consumer && !self.captured) {
            return Err("stale or invalid accuracy child report".into());
        }
        report.validate_identity(
            self.role,
            self.process.child().id(),
            &self.luid,
            self.scenario,
            self.fixture,
        )?;
        self.finished = true;
        Ok(*report)
    }
    pub fn poll(&mut self) -> ProbeResult<Option<Report>> {
        match self.process.try_receive() {
            Ok(Ok(response)) => self.accept(response).map(Some),
            Ok(Err(e)) => Err(e.into()),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("accuracy child exited before reporting".into()),
        }
    }
    pub fn finish(mut self) -> ProbeResult<Report> {
        self.process.owned.request_stop();
        let response = self.process.receive(Duration::from_secs(5))?;
        let report = self.accept(response)?;
        self.join()?;
        Ok(report)
    }
    pub fn join(&mut self) -> ProbeResult<()> {
        if !self.finished {
            return Err("accuracy child joined without terminal report".into());
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.process.child_mut().try_wait()? {
                if !status.success() || self.process.owned.forced_stop {
                    return Err("accuracy child failed shutdown".into());
                }
                self.process.join_reader()?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("accuracy child exit timed out".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
