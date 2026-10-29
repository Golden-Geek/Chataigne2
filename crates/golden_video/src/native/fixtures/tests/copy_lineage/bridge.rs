use std::time::Duration;

use uuid::Uuid;

use super::{ConsumerFrame, descriptor, diagnostics, wait};
use crate::TextureId;
use crate::native::completion::EventCompletion;
use crate::native::device::Device;
use crate::native::fixtures::consumer::log::GpuLog;
use crate::native::fixtures::pipeline_helper::Pipeline;
use crate::native::fixtures::{CardStamp, marker_hlsl};
use crate::native::inputs::{CopyFrame, SourceDemands, SourcePool};
use crate::native::texture::Texture;
use crate::native::timing::{Recorder, SpoutImageVersion};
use crate::native::{AdapterId, D3d11Control, InputLimits};
use crate::plan::{ExternalTexture, PlanLimits, RenderPlan, SpoutFrameCount, SpoutSource};
use crate::runtime::{CompletionToken, DeviceGeneration};

/// Exercises the actual source worker, keyed bridge, import and renderer copy. Its only
/// readback occurs after measurement and actual completion, through the existing GPU log.
pub(super) struct BridgeCapture {
    pool: SourcePool,
    demand: Option<SourceDemands>,
    target: Texture,
    log: GpuLog,
    pipeline: Pipeline,
    completion: EventCompletion,
    device: Device,
    generation: DeviceGeneration,
    submitted: u64,
    previous: CopyFrame,
    records: Vec<(CompletionToken, SpoutImageVersion)>,
}

impl BridgeCapture {
    pub fn new(adapter: AdapterId, sender: &str) -> Self {
        let generation = DeviceGeneration::new(Uuid::new_v4(), 1);
        let device = Device::new(adapter).unwrap();
        let target = Texture::new(&device.device, descriptor()).unwrap();
        let log = GpuLog::new(&device.device, 256).unwrap();
        let shader = format!("{}\n{}", marker_hlsl(), include_str!("../single_source.hlsl"));
        let pipeline = Pipeline::new(&device.device, &shader, 0).unwrap();
        let completion = EventCompletion::new(&device).unwrap();
        let control = D3d11Control::default();
        *control.diagnostics.lock().unwrap() = Some(Recorder::new(diagnostics()));
        let mut pool = SourcePool::new(
            adapter,
            InputLimits {
                max_sources: 1,
                max_storage_bytes: descriptor().byte_len().unwrap() * 5,
            },
            &control,
            None,
            generation,
        );
        let id = TextureId::new(1).unwrap();
        let plan = RenderPlan {
            external_textures: vec![ExternalTexture {
                id,
                descriptor: descriptor(),
                source: Some(
                    SpoutSource {
                        sender: sender.into(),
                        descriptor: descriptor(),
                        poll_interval: Duration::from_millis(1),
                        frame_count: SpoutFrameCount::VersionedV1,
                    }
                    .into(),
                ),
            }],
            ..Default::default()
        }
        .compile_with_source_demands(
            &PlanLimits {
                max_passes: 0,
                max_external_textures: 1,
                max_outputs: 0,
                max_dependencies: 0,
                max_texture_dimension: 3840,
                max_texture_bytes: descriptor().byte_len().unwrap(),
            },
            &[id],
        )
        .unwrap();
        pool.ensure(&plan).unwrap();
        let demand = Some(pool.demands(&plan));
        Self {
            pool,
            demand,
            target,
            log,
            pipeline,
            completion,
            device,
            generation,
            submitted: 0,
            previous: CopyFrame::default(),
            records: Vec::with_capacity(256),
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn tick(&mut self) {
        let done = self.completion.completed(&self.device, self.generation).unwrap();
        self.pool.retire(done.serial);
        if !self.completion.can_submit() {
            return;
        }
        self.submitted += 1;
        let token = CompletionToken {
            device: self.generation,
            serial: self.submitted,
        };
        let _submission = self.completion.begin(&self.device, token);
        let next = self
            .pool
            .copy(
                TextureId::new(1).unwrap(),
                &self.device,
                &self.target,
                self.submitted,
                self.previous,
            )
            .unwrap();
        if let Some(next) = next.filter(|next| next.sequence != 0 && next.sequence != self.previous.sequence) {
            assert!(self.records.len() < 256, "append-only bridge capture exhausted");
            self.pipeline
                .draw_region(
                    &self.device.context,
                    &self.log.target,
                    self.log.origin(self.records.len() as u32),
                    [3, 1],
                    &[Some(self.target.srv.clone())],
                    &[],
                )
                .unwrap();
            self.records.push((token, next.timing.unwrap().spout_image.unwrap()));
            self.previous = next;
        }
    }

    fn drain(&mut self) {
        wait("bridge capture actual completion", || {
            let done = self.completion.completed(&self.device, self.generation).unwrap();
            self.pool.retire(done.serial);
            done.serial >= self.submitted
        });
    }

    pub fn finish(&mut self) -> Vec<ConsumerFrame> {
        self.drain();
        self.submitted += 1;
        {
            let _submission = self.completion.begin(
                &self.device,
                CompletionToken {
                    device: self.generation,
                    serial: self.submitted,
                },
            );
            // SAFETY: one post-window copy of the same-layout GPU log, fenced before Map.
            unsafe {
                self.device.context.CopyResource(&self.log.staging, &self.log.texture);
            }
        }
        self.drain();
        let mut frames = Vec::new();
        self.log
            .read(&self.device.context, self.records.len() as u32, |row, words| {
                let (token, image) = self.records[row as usize];
                frames.push(ConsumerFrame {
                    row,
                    token,
                    spout_image: Some(image),
                    receive_begin_qpc: None,
                    completion_observed_qpc: None,
                    raw_words: words,
                    left: CardStamp::from_words(words[..6].try_into().unwrap()),
                    right: CardStamp::from_words(words[6..].try_into().unwrap()),
                });
            })
            .unwrap();
        frames
    }
}

impl Drop for BridgeCapture {
    fn drop(&mut self) {
        self.demand.take();
        // A failed observation is not a fence. The test watchdog owns the process bound.
        while !self.device.removed() {
            if let Ok(done) = self.completion.completed(&self.device, self.generation) {
                self.pool.retire(done.serial);
                if done.serial >= self.submitted {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
