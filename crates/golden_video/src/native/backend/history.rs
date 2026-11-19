//! Device-local snapshots for delayed texture reads, independent of this frame's draw order.
use std::cell::Cell;
use std::rc::Rc;

use golden_time::MonotonicTime;
use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext;

use crate::LinearColor;
use crate::native::pipeline::Pipeline;
use crate::native::texture::Texture;
use crate::shader::FragmentProgram;

pub(super) struct History {
    pub(super) current: Rc<Texture>,
    pub(super) previous: Rc<Texture>,
    last_sample: Cell<Option<MonotonicTime>>,
    has_draw: Cell<bool>,
    // A compatible plan's index cannot outlive the bytecode address it uses as identity.
    _program: FragmentProgram,
}

impl History {
    pub(super) fn new(current: Rc<Texture>, previous: Rc<Texture>, program: FragmentProgram) -> Self {
        Self {
            current,
            previous,
            last_sample: Cell::new(None),
            has_draw: Cell::new(false),
            _program: program,
        }
    }

    pub(super) fn begin(&self, context: &ID3D11DeviceContext, pipeline: &Pipeline, time: MonotonicTime) {
        if self.last_sample.get().is_some_and(|sample| sample >= time) {
            return;
        }
        self.last_sample.set(Some(time));
        if self.has_draw.get() {
            // SAFETY: two different textures with identical descriptors on this render context.
            // Queue the snapshot before any pass writes its new frame.
            unsafe { context.CopyResource(&self.previous.texture, &self.current.texture) };
        } else {
            pipeline.clear(context, &self.previous, LinearColor::TRANSPARENT);
        }
    }

    pub(super) fn written(&self) {
        self.has_draw.set(true);
    }
}
