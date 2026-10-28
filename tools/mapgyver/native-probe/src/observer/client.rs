use mapgyver_native_probe::frame_age::{self, Config, Sample};
use std::process::Command;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use super::protocol::{Checkpoint, CompositePhase, ObserverReport, Request, Response, request_line};
use crate::{ProbeResult, process::ReportedProcess};
use mapgyver_native_probe::resource_log::Stream;
use mapgyver_native_probe::workload::Scenario;

pub struct ObserverProcess {
    process: ReportedProcess<Response>,
    resources: Stream,
    requested: usize,
    checkpoints: Vec<Checkpoint>,
    completed: Option<ObserverReport>,
    scenario: Scenario,
    generation_requested: u32,
    generations: Vec<Checkpoint>,
    age_config: Option<Config>,
    age_samples: Vec<Sample>,
}

impl ObserverProcess {
    pub fn child(&self) -> &std::process::Child {
        self.process.child()
    }

    pub fn spawn(adapter: u32, name: &str, expected_luid: &str, scenario: Scenario) -> ProbeResult<Self> {
        let process: ReportedProcess<Response> = ReportedProcess::spawn(
            Command::new(std::env::current_exe()?).args(["observe", &adapter.to_string(), name, scenario.name()]),
            request_line(&Request::Finish)?,
        )?;
        match process.receive(Duration::from_secs(5))? {
            Response::Ready {
                process_id,
                adapter_luid,
            } if process_id == process.child().id() && adapter_luid == expected_luid => {}
            _ => return Err("observer did not initialize on the selected adapter".into()),
        }
        Ok(Self {
            process,
            resources: Stream::default(),
            requested: 0,
            checkpoints: Vec::with_capacity(4),
            completed: None,
            scenario,
            generation_requested: 0,
            generations: Vec::new(),
            age_config: None,
            age_samples: Vec::new(),
        })
    }

    pub fn sample(&mut self, phase: CompositePhase) -> ProbeResult<()> {
        if CompositePhase::ALL.get(self.requested) != Some(&phase) {
            return Err("invalid observer request sequence".into());
        }
        self.process.owned.send(&request_line(&Request::Sample { phase })?)?;
        self.requested += 1;
        Ok(())
    }

    pub fn poll(&mut self) -> ProbeResult<()> {
        loop {
            match self.process.try_receive() {
                Ok(Ok(Response::Resources { message })) if self.completed.is_none() => {
                    self.resources.accept(message)?
                }
                Ok(Ok(Response::Resources { .. })) => return Err("resources after observer completion".into()),
                Ok(Ok(Response::Checkpoint { result })) => {
                    if CompositePhase::ALL.get(self.checkpoints.len()) != Some(&result.phase)
                        || !self.valid_checkpoint(&result)
                    {
                        return Err("observer returned an invalid checkpoint".into());
                    }
                    self.checkpoints.push(result);
                }
                Ok(Ok(Response::Generation { generation, result })) => {
                    if generation != self.generation_requested
                        || generation != self.generations.len() as u32 + 1
                        || result.phase != CompositePhase::Recovered
                        || !self.valid_checkpoint(&result)
                        || result.sampled_generation_markers.last()
                            != Some(&super::protocol::generation_marker(generation))
                    {
                        return Err("observer returned stale or invalid generation content".into());
                    }
                    self.generations.push(result);
                }
                Ok(Ok(Response::FrameAgeChunk { offset, samples })) => {
                    let config = self.age_config.ok_or("unrequested frame-age data")?;
                    if self.completed.is_some()
                        || offset != self.age_samples.len()
                        || samples.is_empty()
                        || samples.len() > frame_age::CHUNK_SAMPLES
                        || offset + samples.len() > config.capacity()
                    {
                        return Err("invalid frame-age chunk sequence/bounds".into());
                    }
                    self.age_samples.extend(samples);
                }
                Ok(Ok(Response::Finished { mut report })) => {
                    if self.completed.is_some()
                        || report.checkpoints != self.checkpoints
                        || self.checkpoints.len() != 4
                        || report.verified_generations != self.generations.len() as u32
                        || report.verified_generations != self.generation_requested
                        || report.resources.is_some()
                    {
                        return Err("observer completion omitted or replaced checkpoints".into());
                    }
                    report.resources = Some(self.resources.finish()?);
                    match (&mut report.frame_age, self.age_config) {
                        (Some(age), Some(config))
                            if age.config == config
                                && age.samples.is_empty()
                                && age.captured_samples == self.age_samples.len()
                                && age.marker_copy_pixels == age.captured_samples * frame_age::PIXELS_PER_SAMPLE
                                && age.log_readbacks == 1 =>
                        {
                            if age.channels != frame_age::analyze(config, &self.age_samples)? {
                                return Err("frame-age summary disagrees with raw samples".into());
                            }
                            age.samples = std::mem::take(&mut self.age_samples);
                        }
                        (None, None) => {}
                        _ => return Err("observer omitted or replaced frame-age evidence".into()),
                    }
                    self.completed = Some(*report);
                }
                Ok(Ok(Response::Ready { .. })) => return Err("observer repeated initialization".into()),
                Ok(Err(error)) => return Err(error.into()),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) if self.completed.is_some() => break,
                Err(TryRecvError::Disconnected) => return Err("observer exited before completing validation".into()),
            }
        }
        Ok(())
    }

    fn valid_checkpoint(&self, result: &Checkpoint) -> bool {
        result.extent == self.scenario.extent()
            && !result.sampled_bgra.is_empty()
            && result.sampled_bgra.len() <= 4
            && result.sampled_bgra.last() == Some(&result.phase.expected_pixels())
            && (!self.scenario.cards()
                || (result.sampled_alpha.len() == result.sampled_bgra.len()
                    && result.sampled_alpha.last() == Some(&super::protocol::expected_alpha(result.phase))))
    }

    pub fn recovered(&self) -> bool {
        self.checkpoints.len() == 4
    }

    pub fn begin_frame_age(&mut self, config: Config) -> ProbeResult<()> {
        config.validate()?;
        if self.age_config.is_some() || self.requested != 0 || !self.scenario.cards() {
            return Err("invalid frame-age request admission".into());
        }
        self.process
            .owned
            .send(&request_line(&Request::BeginFrameAge { config })?)?;
        self.age_config = Some(config);
        Ok(())
    }
    pub fn verified_generation(&self) -> u32 {
        self.generations.len() as u32
    }
    pub fn generation_result(&self, generation: u32) -> Option<Checkpoint> {
        generation
            .checked_sub(1)
            .and_then(|index| self.generations.get(index as usize))
            .cloned()
    }
    pub fn verify_generation(&mut self, generation: u32) -> ProbeResult<()> {
        if !self.recovered()
            || generation > 100
            || generation != self.generation_requested + 1
            || self.generation_requested != self.verified_generation()
        {
            return Err("invalid generation request admission".into());
        }
        self.process
            .owned
            .send(&request_line(&Request::VerifyGeneration { generation })?)?;
        self.generation_requested = generation;
        Ok(())
    }

    pub fn finish(mut self) -> ProbeResult<ObserverReport> {
        if self.requested != 4 {
            return Err("render run ended before all composite phases".into());
        }
        self.process.owned.request_stop();
        let deadline = Instant::now() + Duration::from_secs(5);
        // Drain while the child writes chunks. Waiting for exit first deadlocks once the
        // bounded channel and OS stdout pipe fill; neither buffer grows with run duration.
        let status = loop {
            self.poll()?;
            if let Some(status) = self.process.child_mut().try_wait()?
                && self.completed.is_some()
            {
                break status;
            }
            if Instant::now() >= deadline {
                return Err("observer finish exceeded five-second drain deadline".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        if self.process.owned.forced_stop || !status.success() {
            return Err(format!("observer did not exit cleanly: {status}").into());
        }
        self.process.join_reader()?;
        self.poll()?;
        Ok(self.completed.take().ok_or("observer completion missing")?)
    }
}
