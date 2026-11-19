use std::{cell::Cell, rc::Rc};

use golden_time::TimeSnapshot;
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::Result;

use super::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Pipeline, RECT, Texture, missing_output};
use crate::LinearColor;
use crate::shader::FragmentProgram;

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
    last_frame: Cell<Option<FrameSample>>,
}

impl ShaderPass {
    pub(in crate::native) fn new(draw: Rc<ShaderDraw>) -> Self {
        Self {
            draw,
            last_frame: Cell::new(None),
        }
    }

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
    // Keep the bytecode identity unique while this draw is reusable across plans.
    _program: FragmentProgram,
}

impl ShaderDraw {
    pub(in crate::native) fn new(device: &ID3D11Device, program: &FragmentProgram) -> Result<Self> {
        let mut pixel = None;
        let mut frame = None;
        // SAFETY: the compiler produced a validated pixel shader and the descriptor matches b0.
        unsafe {
            device.CreatePixelShader(program.bytecode(), None, Some(&mut pixel))?;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: size_of::<FrameConstants>() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut frame),
            )?;
        }
        Ok(Self {
            pixel: pixel.ok_or_else(missing_output)?,
            frame: frame.ok_or_else(missing_output)?,
            _program: program.clone(),
        })
    }
}

impl Pipeline {
    pub(in crate::native) fn draw_shader(
        &self,
        context: &ID3D11DeviceContext,
        target: &Texture,
        pass: &ShaderPass,
        time: TimeSnapshot,
    ) {
        let extent = target.descriptor.extent;
        let width = extent.width();
        let height = extent.height();
        let seconds = time.show_position().as_nanos() as f32 * 1e-9;
        let sample = pass.sample(time);
        let rate = if sample.delta_seconds > 0.0 {
            sample.delta_seconds.recip()
        } else {
            0.0
        };
        let frame = FrameConstants {
            size_and_time: [width as f32, height as f32, 1.0, seconds],
            timing: [sample.delta_seconds, rate, 0.0, 0.0],
            indices: [sample.index, 0, 0, 0],
        };
        let shader = &pass.draw;
        // A legal fragment can discard a pixel, which leaves that pixel transparent.
        self.clear(context, target, LinearColor::TRANSPARENT);
        // SAFETY: the shader, constant buffer, context and target belong to the same device.
        unsafe {
            context.UpdateSubresource(&shader.frame, 0, None, std::ptr::from_ref(&frame).cast(), 0, 0);
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.GSSetShader(None, None);
            context.HSSetShader(None, None);
            context.DSSetShader(None, None);
            context.PSSetShader(&shader.pixel, None);
            context.PSSetConstantBuffers(0, Some(&[Some(shader.frame.clone())]));
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
            context.PSSetConstantBuffers(0, Some(&[None]));
            context.OMSetRenderTargets(None, None);
        }
    }
}
