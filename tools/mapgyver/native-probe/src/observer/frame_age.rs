//! GPU-only bounded image log. One final staging map, on this diagnostic process only.

use crate::{ProbeResult, clock, d3d::Gpu, spout::Endpoint, texture::Texture};
use mapgyver_native_probe::allocation::{Owner, Work};
use mapgyver_native_probe::frame_age::{self, PIXELS_PER_SAMPLE, Report, Sample};
use mapgyver_native_probe::sample_window::Window;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use windows::Win32::Graphics::{Direct3D11::*, Dxgi::Common::*};

const RECORDS_PER_ROW: usize = 64;
const PERIOD: Duration = Duration::from_nanos(16_666_667);

pub struct Capture {
    submitted: u64,
    completed: Option<u64>,
}

pub struct Collector {
    config: Window,
    log: Texture,
    queries: Vec<ID3D11Query>,
    pending: VecDeque<(usize, usize)>,
    captures: Vec<Capture>,
    next_sample: Instant,
    no_new_frame: u32,
    ring_full: u32,
    late_sampling_ticks: u32,
}

impl Collector {
    pub fn new(gpu: &Gpu, config: Window) -> ProbeResult<Self> {
        config.validate()?;
        if config.frequency != clock::frequency()? {
            return Err("observer/renderer QPC frequency mismatch".into());
        }
        let log = gpu.texture(
            (RECORDS_PER_ROW * PIXELS_PER_SAMPLE) as u32,
            config.capacity().div_ceil(RECORDS_PER_ROW) as u32,
        )?;
        let mut queries = Vec::with_capacity(4);
        for _ in 0..4 {
            let mut query = None;
            // SAFETY: event queries are retained until their corresponding copies complete.
            unsafe {
                gpu.device.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: D3D11_QUERY_EVENT,
                        MiscFlags: 0,
                    },
                    Some(&mut query),
                )?;
            }
            queries.push(query.ok_or("missing frame-age event query")?);
        }
        Ok(Self {
            config,
            log,
            queries,
            pending: VecDeque::with_capacity(4),
            captures: Vec::with_capacity(config.capacity()),
            next_sample: Instant::now(),
            no_new_frame: 0,
            ring_full: 0,
            late_sampling_ticks: 0,
        })
    }

    fn poll(&mut self, gpu: &Gpu) -> ProbeResult<()> {
        while let Some(&(slot, index)) = self.pending.front() {
            if !crate::query_data::event(&gpu.context, &self.queries[slot])? {
                break;
            }
            // This is after GPU copy completion, never the CPU's earlier copy enqueue time.
            self.captures[index].completed = Some(clock::now()?);
            self.pending.pop_front();
        }
        Ok(())
    }

    pub fn tick(&mut self, gpu: &Gpu, receiver: &mut Endpoint<'_>, extent: [u32; 2]) -> ProbeResult<()> {
        self.poll(gpu)?;
        let now = Instant::now();
        if now < self.next_sample {
            return Ok(());
        }
        let collecting = self.config.collecting(clock::now()?);
        let skipped = now.duration_since(self.next_sample).as_nanos() / PERIOD.as_nanos();
        if collecting {
            self.late_sampling_ticks += skipped as u32;
        }
        // Preserve phase through ordinary wakeup jitter; skip whole overdue periods instead
        // of accumulating drift or issuing a catch-up burst after diagnostic checkpoints.
        self.next_sample += PERIOD;
        if self.next_sample <= now {
            self.next_sample = now + PERIOD;
        }
        if !collecting {
            return Ok(());
        }
        if self.captures.len() == self.config.capacity() {
            return Err("frame-age GPU log capacity exhausted".into());
        }
        let Some(slot) = (0..self.queries.len()).find(|slot| !self.pending.iter().any(|p| p.0 == *slot)) else {
            self.ring_full += 1;
            return Ok(());
        };
        let Some(frame) = receiver.receive()? else {
            self.no_new_frame += 1;
            return Ok(());
        };
        if [frame.width, frame.height] != extent {
            return Err("frame-age output extent changed".into());
        }
        let submitted = clock::now()?;
        if !self.config.collecting(submitted) {
            return Ok(());
        }
        let index = self.captures.len();
        let x = (index % RECORDS_PER_ROW * PIXELS_PER_SAMPLE) as u32;
        let y = (index / RECORDS_PER_ROW) as u32;
        // SAFETY: ten isolated pixels go to unique in-bounds cells on the same device.
        // The SDK receiver's private texture and this log share the ordered immediate context.
        unsafe {
            for channel in 0..2 {
                for word in 0..5 {
                    gpu.allocations.work(Work::GpuCopy(4));
                    let source_x = extent[0] / 2 * channel + extent[0] / 2 * (3 + word * 2) / 100;
                    let source_y = extent[1] * 26 / 100;
                    gpu.context.CopySubresourceRegion(
                        &*self.log,
                        0,
                        x + channel * 5 + word,
                        y,
                        0,
                        &*frame.texture,
                        0,
                        Some(&D3D11_BOX {
                            left: source_x,
                            top: source_y,
                            front: 0,
                            right: source_x + 1,
                            bottom: source_y + 1,
                            back: 1,
                        }),
                    );
                }
            }
            gpu.context.End(&self.queries[slot]);
            gpu.context.Flush();
        }
        self.captures.push(Capture {
            submitted,
            completed: None,
        });
        self.pending.push_back((slot, index));
        Ok(())
    }

    pub fn finish(mut self, gpu: &Gpu) -> ProbeResult<Report> {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !self.pending.is_empty() {
            self.poll(gpu)?;
            if Instant::now() >= deadline {
                return Err("frame-age capture completion timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let pixels = self.read_log(gpu)?;
        let mut samples = Vec::with_capacity(self.captures.len());
        for (index, capture) in self.captures.iter().enumerate() {
            let mut words = [[0; 5]; 2];
            for (word, pixel) in words.iter_mut().flatten().zip(&pixels[index]) {
                *word = u32::from_le_bytes([pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
            samples.push(Sample {
                submitted_qpc: capture.submitted,
                completed_qpc: capture.completed.ok_or("missing frame-age copy completion")?,
                words,
            });
        }
        let channels = frame_age::analyze(self.config, &samples)?;
        Ok(Report {
            config: self.config,
            captured_samples: samples.len(),
            marker_copy_pixels: samples.len() * PIXELS_PER_SAMPLE,
            final_log_copy_bytes: self.config.capacity().div_ceil(RECORDS_PER_ROW)
                * RECORDS_PER_ROW
                * PIXELS_PER_SAMPLE
                * 4,
            log_readbacks: 1,
            no_new_frame: self.no_new_frame,
            ring_full: self.ring_full,
            late_sampling_ticks: self.late_sampling_ticks,
            channels,
            samples,
        })
    }

    fn read_log(&self, gpu: &Gpu) -> ProbeResult<Vec<[[u8; 4]; PIXELS_PER_SAMPLE]>> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: writable descriptor of our retained log texture.
        unsafe { self.log.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut staging = None;
        // SAFETY: staging format/extent exactly matches the GPU-only log.
        unsafe { gpu.device.CreateTexture2D(&desc, None, Some(&mut staging))? };
        let staging = staging.ok_or("missing frame-age staging log")?;
        let staging = Texture::new(staging, &gpu.allocations, Owner::Application)?;
        gpu.allocations
            .work(Work::Readback((self.captures.len() * PIXELS_PER_SAMPLE) as u64));
        gpu.allocations
            .work(Work::GpuCopy(u64::from(desc.Width) * u64::from(desc.Height) * 4));
        // SAFETY: resources have identical dimensions/formats and belong to this device.
        unsafe { gpu.context.CopyResource(&*staging, &*self.log) };
        gpu.wait_submitted()?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let mut pixels = vec![[[0; 4]; PIXELS_PER_SAMPLE]; self.captures.len()];
        // SAFETY: one post-fence map; row pitch is checked before any bounded CPU access.
        unsafe {
            gpu.context.Map(&*staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            if mapped.pData.is_null() || mapped.RowPitch < desc.Width * 4 || desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
                gpu.context.Unmap(&*staging, 0);
                return Err("invalid frame-age staging layout".into());
            }
            for (index, record) in pixels.iter_mut().enumerate() {
                let offset = index / RECORDS_PER_ROW * mapped.RowPitch as usize
                    + index % RECORDS_PER_ROW * PIXELS_PER_SAMPLE * 4;
                std::ptr::copy_nonoverlapping(
                    mapped.pData.cast::<u8>().add(offset),
                    record.as_mut_ptr().cast::<u8>(),
                    PIXELS_PER_SAMPLE * 4,
                );
            }
            gpu.context.Unmap(&*staging, 0);
        }
        Ok(pixels)
    }
}
