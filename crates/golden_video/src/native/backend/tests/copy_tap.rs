//! Test-only GPU taps. All shaders/logs/queries remain on the renderer, with one teardown Map.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext;
use windows::core::Result;

use crate::native::completion::EventCompletion;
use crate::native::device::Device;
use crate::native::fixtures::GpuLog;
use crate::native::fixtures::marker_hlsl;
use crate::native::fixtures::pipeline_helper::Pipeline;
use crate::native::texture::Texture;
use crate::runtime::CompletionToken;

const CAPACITY: u32 = 256;

#[derive(Clone, Default)]
pub(in crate::native) struct Control(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    enabled: bool,
    result: Option<std::result::Result<Vec<Frame>, String>>,
}

#[derive(Clone)]
pub(in crate::native) struct Frame {
    pub token: CompletionToken,
    pub words: [u32; 12],
}

impl Control {
    pub fn enable(&self) {
        self.0.lock().unwrap().enabled = true;
    }
    pub fn take(&self) -> Vec<Frame> {
        self.0.lock().unwrap().result.take().expect("tap finalized").unwrap()
    }
}

pub(in crate::native) struct Tap {
    log: GpuLog,
    pipeline: Pipeline,
    completion: EventCompletion,
    tokens: Vec<CompletionToken>,
    control: Control,
    overflow: bool,
}

impl Tap {
    pub fn new(device: &Device, control: Control) -> Result<Option<Self>> {
        if !control.0.lock().unwrap().enabled {
            return Ok(None);
        }
        let shader = format!("{}\n{}", marker_hlsl(), include_str!("copy_tap.hlsl"));
        Ok(Some(Self {
            log: GpuLog::new(&device.device, CAPACITY)?,
            pipeline: Pipeline::new(&device.device, &shader, 0)?,
            completion: EventCompletion::new(device)?,
            tokens: Vec::with_capacity(CAPACITY as usize),
            control,
            overflow: false,
        }))
    }

    pub fn record(&mut self, context: &ID3D11DeviceContext, input: &Texture, output: &Texture, token: CompletionToken) {
        if self.tokens.len() == CAPACITY as usize {
            self.overflow = true;
            return;
        }
        self.pipeline
            .draw_region(
                context,
                &self.log.target,
                self.log.origin(self.tokens.len() as u32),
                [3, 1],
                &[Some(input.srv.clone()), Some(output.srv.clone())],
                &[],
            )
            .unwrap();
        self.tokens.push(token);
    }

    pub fn finish(&mut self, device: &Device) {
        let result = (|| -> std::result::Result<Vec<Frame>, String> {
            if self.overflow || self.tokens.is_empty() {
                return Err("tap coverage missing or exhausted".into());
            }
            if device.removed() {
                return Err("device removed before tap readback".into());
            }
            let token = *self.tokens.last().unwrap();
            {
                let _submission = self.completion.begin(device, token);
                // SAFETY: Core completed all render uses before NativeState retirement. This
                // last same-layout staging copy gets its own preallocated event before Map.
                unsafe {
                    device.context.CopyResource(&self.log.staging, &self.log.texture);
                }
            }
            loop {
                if self
                    .completion
                    .completed(device, token.device)
                    .is_ok_and(|done| done.serial >= token.serial)
                {
                    break;
                }
                if device.removed() {
                    return Err("device removed during tap readback".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let mut rows = Vec::with_capacity(self.tokens.len());
            self.log
                .read(&device.context, self.tokens.len() as u32, |row, words| {
                    rows.push(Frame {
                        token: self.tokens[row as usize],
                        words,
                    });
                })
                .map_err(|e| e.to_string())?;
            Ok(rows)
        })();
        self.control.0.lock().unwrap().result = Some(result);
    }
}
