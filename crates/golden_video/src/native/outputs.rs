use std::collections::BTreeMap;

use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};
use windows::core::{Error, HRESULT, Result};

use crate::{OutputId, TextureDescriptor};

use super::config::{D3d11Config, OutputBinding};
use super::texture::Texture;

mod spout;
mod window;

/// Output-local delivery does not invalidate commands already submitted for other consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Delivery {
    Accepted,
    Busy,
    Unavailable,
    Failed(HRESULT),
}

enum Output {
    Window(window::WindowOutput),
    Spout(spout::SpoutOutput),
}

struct Binding {
    descriptor: TextureDescriptor,
    output: Output,
}

/// One persistent output pool per device, independently of prepared/live plan generations.
pub(super) struct OutputPool {
    bindings: BTreeMap<OutputId, Binding>,
}

impl OutputPool {
    /// The caller reserves all persistent native backing storage before construction.
    pub(super) fn new(device: &ID3D11Device, config: D3d11Config) -> Result<Self> {
        let mut bindings = BTreeMap::new();
        for binding in config.outputs {
            let id = binding.id();
            if bindings.contains_key(&id) {
                return Err(Error::new(
                    HRESULT(0x8007_0057_u32 as i32),
                    "duplicate native output identity",
                ));
            }
            let descriptor = binding.descriptor();
            let output = match binding {
                OutputBinding::Window { window, .. } => {
                    Output::Window(window::WindowOutput::new(device, window, descriptor)?)
                }
                OutputBinding::Spout { name, .. } => Output::Spout(spout::SpoutOutput::new(device, name, descriptor)?),
            };
            bindings.insert(id, Binding { descriptor, output });
        }
        Ok(Self { bindings })
    }

    pub(super) fn descriptor(&self, id: OutputId) -> Option<TextureDescriptor> {
        self.bindings.get(&id).map(|binding| binding.descriptor)
    }

    pub(super) fn deliver(&mut self, id: OutputId, context: &ID3D11DeviceContext, texture: &Texture) -> Delivery {
        let Some(binding) = self.bindings.get_mut(&id) else {
            return Delivery::Unavailable;
        };
        if texture.descriptor != binding.descriptor {
            return Delivery::Failed(HRESULT(0x8007_0057_u32 as i32));
        }
        match &mut binding.output {
            Output::Window(output) => output.deliver(context, texture),
            Output::Spout(output) => output.deliver(texture),
        }
    }
}
