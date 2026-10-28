use super::protocol::{Report, Request, Response};
use crate::{ProbeResult, process::ReportedProcess, wire};
use mapgyver_native_probe::{resource_log::Stream, workload::Scenario};
use std::process::{Child, Command};
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

pub struct FixtureProcess {
    process: ReportedProcess<Response>,
    channel: String,
    luid: String,
    resources: Stream,
    completed: Option<Report>,
}

pub fn spawn_ready(
    adapter: u32,
    name: &str,
    channel: &str,
    frames: u32,
    scenario: Scenario,
    expected_luid: &str,
) -> ProbeResult<FixtureProcess> {
    let process: ReportedProcess<Response> = ReportedProcess::spawn(
        Command::new(std::env::current_exe()?).args([
            "fixture",
            &adapter.to_string(),
            name,
            channel,
            &frames.to_string(),
            scenario.name(),
        ]),
        wire::line(&Request::Stop)?,
    )?;
    match process.receive(Duration::from_secs(5))? {
        Response::Ready {
            process_id,
            adapter_luid,
            channel: actual,
        } if process_id == process.child().id() && adapter_luid == expected_luid && actual == channel => {}
        _ => return Err("source did not confirm its first submitted image on the selected adapter".into()),
    }
    Ok(FixtureProcess {
        process,
        channel: channel.into(),
        luid: expected_luid.into(),
        resources: Stream::default(),
        completed: None,
    })
}

impl FixtureProcess {
    pub fn child(&self) -> &Child {
        self.process.child()
    }
    pub fn child_mut(&mut self) -> &mut Child {
        self.process.child_mut()
    }

    fn poll(&mut self) -> ProbeResult<()> {
        loop {
            match self.process.try_receive() {
                Ok(Ok(Response::Resources { message })) if self.completed.is_none() => {
                    self.resources.accept(message)?
                }
                Ok(Ok(Response::Finished { mut report }))
                    if self.completed.is_none()
                        && report.process_id == self.child().id()
                        && report.adapter_luid == self.luid
                        && report.channel == self.channel
                        && report.resources.is_none() =>
                {
                    report.resources = Some(self.resources.finish()?);
                    self.completed = Some(*report);
                }
                Ok(Ok(_)) => return Err("invalid source report sequence/identity".into()),
                Ok(Err(error)) => return Err(error.into()),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) if self.completed.is_some() => break,
                Err(TryRecvError::Disconnected) => return Err("source exited without its resource report".into()),
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> ProbeResult<Report> {
        self.process.owned.request_stop();
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            self.poll()?;
            if let Some(status) = self.process.child_mut().try_wait()?
                && self.completed.is_some()
            {
                break status;
            }
            if Instant::now() >= deadline {
                return Err("source resource drain exceeded five seconds".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        if !status.success() || self.process.owned.forced_stop {
            return Err("source did not retire cleanly".into());
        }
        self.process.join_reader()?;
        self.poll()?;
        self.completed.take().ok_or_else(|| "source completion missing".into())
    }
}
