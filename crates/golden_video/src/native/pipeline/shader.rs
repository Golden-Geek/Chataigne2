use std::{cell::Cell, rc::Rc};

use golden_time::TimeSnapshot;
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::Result;

use super::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Pipeline, RECT, Texture, missing_output};
use crate::shader::{FragmentProgram, TEXTURE_BINDINGS};

#[repr(C)]
struct FrameConstants {
    size_and_time: [f32; 4],
    timing: [f32; 4],
    indices: [i32; 4],
}

#[derive(Clone, Copy, Default)]
struct FrameSample {
    monotonic_ns: u64,
    index: i32,
    delta_seconds: f32,
}

pub(in crate::native) struct ShaderPass {
    draw: Rc<ShaderDraw>,
    clock: Rc<ShaderClock>,
    program: FragmentProgram,
}

impl ShaderPass {
    pub(in crate::native) fn new(draw: Rc<ShaderDraw>, clock: Rc<ShaderClock>, program: &FragmentProgram) -> Self {
        Self {
            draw,
            clock,
            program: program.clone(),
        }
    }
}

#[derive(Default)]
pub(in crate::native) struct ShaderClock {
    last_frame: Cell<Option<FrameSample>>,
}

impl ShaderClock {
    fn sample(&self, time: TimeSnapshot) -> FrameSample {
        let monotonic_ns = time.monotonic().as_nanos();
        let sample = match self.last_frame.get() {
            Some(previous) if monotonic_ns <= previous.monotonic_ns => previous,
            Some(previous) => FrameSample {
                monotonic_ns,
                index: previous.index.saturating_add(1),
                delta_seconds: (monotonic_ns - previous.monotonic_ns) as f32 * 1e-9,
            },
            None => FrameSample {
                monotonic_ns,
                ..Default::default()
            },
        };
        self.last_frame.set(Some(sample));
        sample
    }
}

pub(in crate::native) struct ShaderDraw {
    pixel: ID3D11PixelShader,
    frame: ID3D11Buffer,
    uniforms: Option<ID3D11Buffer>,
    // Keep the bytecode identity unique while this draw is reusable across plans.
    _program: FragmentProgram,
}

impl ShaderDraw {
    pub(in crate::native) fn new(device: &ID3D11Device, program: &FragmentProgram) -> Result<Self> {
        let mut pixel = None;
        // SAFETY: the compiler produced a validated pixel shader.
        unsafe {
            device.CreatePixelShader(program.bytecode(), None, Some(&mut pixel))?;
        }
        let uniform_bytes = size_of_val(program.uniform_values());
        Ok(Self {
            pixel: pixel.ok_or_else(missing_output)?,
            frame: constant_buffer(device, size_of::<FrameConstants>())?,
            uniforms: (uniform_bytes != 0)
                .then(|| constant_buffer(device, uniform_bytes))
                .transpose()?,
            _program: program.clone(),
        })
    }
}

fn constant_buffer(device: &ID3D11Device, bytes: usize) -> Result<ID3D11Buffer> {
    let mut buffer = None;
    // SAFETY: the descriptor uses a nonzero multiple of 16 bytes within D3D11's constant buffer limit.
    unsafe {
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: bytes as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut buffer),
        )?;
    }
    buffer.ok_or_else(missing_output)
}

impl Pipeline {
    pub(in crate::native) fn draw_shader(
        &self,
        context: &ID3D11DeviceContext,
        target: &Texture,
        pass: &ShaderPass,
        inputs: [Option<&Texture>; TEXTURE_BINDINGS],
        pass_index: u8,
        time: TimeSnapshot,
    ) {
        let extent = target.descriptor.extent;
        let width = extent.width();
        let height = extent.height();
        let seconds = time.show_position().as_nanos() as f32 * 1e-9;
        let sample = pass.clock.sample(time);
        let rate = if sample.delta_seconds > 0.0 {
            sample.delta_seconds.recip()
        } else {
            0.0
        };
        let frame = FrameConstants {
            size_and_time: [width as f32, height as f32, 1.0, seconds],
            timing: [sample.delta_seconds, rate, 0.0, 0.0],
            indices: [sample.index, i32::from(pass_index), 0, 0],
        };
        let shader = &pass.draw;
        // A legal fragment can discard a pixel, which leaves that pixel transparent.
        target.clear_zero(context);
        // SAFETY: the shader, constant buffer, context and target belong to the same device.
        unsafe {
            context.UpdateSubresource(&shader.frame, 0, None, std::ptr::from_ref(&frame).cast(), 0, 0);
            if let Some(buffer) = &shader.uniforms {
                let values = pass.program.uniform_values();
                context.UpdateSubresource(buffer, 0, None, values.as_ptr().cast(), 0, 0);
            }
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.GSSetShader(None, None);
            context.HSSetShader(None, None);
            context.DSSetShader(None, None);
            context.PSSetShader(&shader.pixel, None);
            context.PSSetConstantBuffers(0, Some(&[Some(shader.frame.clone()), shader.uniforms.clone()]));
            context.PSSetShaderResources(0, Some(&inputs.map(|input| input.map(|texture| texture.srv.clone()))));
            context.OMSetDepthStencilState(None, 0);
            context.OMSetBlendState(None, None, u32::MAX);
            context.OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            context.RSSetState(&self.rasterizer);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: width as f32,
                Height: height as f32,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            context.RSSetScissorRects(Some(&[RECT {
                right: width as i32,
                bottom: height as i32,
                ..Default::default()
            }]));
            context.Draw(3, 0);
            context.PSSetShader(None, None);
            context.PSSetConstantBuffers(0, Some(&[None, None]));
            context.PSSetShaderResources(0, Some(&[const { None }; TEXTURE_BINDINGS]));
            context.OMSetRenderTargets(None, None);
        }
    }
}
