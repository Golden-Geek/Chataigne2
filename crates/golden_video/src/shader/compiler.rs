use std::ffi::CString;
use std::fmt;
use std::sync::Arc;

use naga::{AddressSpace, Binding, BuiltIn, ResourceBinding, ShaderStage, TypeInner, back::hlsl, front::glsl, valid};
use windows::Win32::Graphics::Direct3D::{Fxc::D3DCompile, ID3DBlob};
use windows::core::{PCSTR, s};

use super::{FragmentFormat, MAX_SOURCE_BYTES, TEXTURE_BINDINGS, TEXTURE_INPUTS, adapter};

const MAX_BYTECODE_BYTES: usize = 256 * 1024;
const FRAME_BINDING: ResourceBinding = ResourceBinding { group: 0, binding: 0 };

/// A validated device-independent fragment program. Cloning shares its bytecode.
#[derive(Clone)]
pub struct FragmentProgram {
    bytecode: Arc<[u8]>,
    sampled_bindings: [bool; TEXTURE_BINDINGS],
    effect: uuid::Uuid,
    buffers: adapter::ShaderToyBuffers,
}

impl fmt::Debug for FragmentProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FragmentProgram")
            .field("bytecode_bytes", &self.bytecode.len())
            .field("sampled_bindings", &self.sampled_bindings)
            .finish()
    }
}

/// Compilation diagnostics suitable for presentation beside the source.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// An empty program or a program exceeding the source or binary limit.
    #[error("Shader source is empty or exceeds the shader size limit")]
    Size,
    /// The source failed parsing, validation or native shader compilation.
    #[error("{0}")]
    Invalid(String),
}

impl FragmentProgram {
    /// Compile a GLSL 450 fragment body away from the render and engine threads.
    ///
    /// Define `void main()` and write straight linear RGBA to the supplied `fragColor`.
    /// `iResolution` supplies (width, height, 1) and `iTime` is show time in seconds.
    /// `gl_FragCoord` and sampled media coordinates follow the upper-left origin.
    pub fn compile_glsl(source: &str) -> Result<Self, CompileError> {
        Self::compile(FragmentFormat::Glsl, source)
    }

    /// Compile a standalone draw. Use [super::FragmentEffect] for a multi-pass source.
    pub fn compile(format: FragmentFormat, source: &str) -> Result<Self, CompileError> {
        if source.trim().is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err(CompileError::Size);
        }
        let (glsl, stages) = adapter::fragment_source(format, source)?;
        if stages.len() != 1 || stages[0].inputs[TEXTURE_INPUTS..].iter().any(Option::is_some) {
            return Err(CompileError::Invalid(
                "Use a FragmentEffect for a multi-pass or feedback shader".into(),
            ));
        }
        Self::compile_source(&glsl, uuid::Uuid::new_v4(), Default::default())
    }

    pub(super) fn compile_source(
        glsl: &str,
        effect: uuid::Uuid,
        buffers: adapter::ShaderToyBuffers,
    ) -> Result<Self, CompileError> {
        let module = glsl::Frontend::default()
            .parse(&glsl::Options::from(ShaderStage::Fragment), glsl)
            .map_err(|error| CompileError::Invalid(error.emit_to_string(glsl)))?;
        let entry = &module.entry_points[0].function;
        if entry.arguments.iter().any(|argument| {
            !matches!(
                argument.binding,
                Some(Binding::BuiltIn(BuiltIn::Position { .. } | BuiltIn::FrontFacing))
            )
        }) || !entry.result.as_ref().is_some_and(|result| {
            matches!(&module.types[result.ty].inner, TypeInner::Struct { members, .. }
                if members.len() == 1 && matches!(members[0].binding, Some(Binding::Location { location: 0, .. })))
        }) {
            return Err(CompileError::Invalid(
                "Expected a single color output and no custom stage inputs".into(),
            ));
        }
        let info = valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::empty())
            .validate(&module)
            .map_err(|error| CompileError::Invalid(error.emit_to_string(glsl)))?;
        let mut options = hlsl::Options {
            shader_model: hlsl::ShaderModel::V5_0,
            fake_missing_bindings: false,
            ..Default::default()
        };
        options.binding_map.insert(FRAME_BINDING, hlsl::BindTarget::default());
        let mut sampled_bindings = [false; TEXTURE_BINDINGS];
        for (handle, global) in module.global_variables.iter() {
            if global.space == AddressSpace::Private
                || (global.space == AddressSpace::Uniform
                    && global.name.as_deref() == Some("goldenFrame")
                    && global.binding == Some(FRAME_BINDING))
            {
                continue;
            }
            let Some(slot) = texture_slot(&module, global) else {
                return Err(CompileError::Invalid("Unsupported GLSL resource binding".into()));
            };
            let binding = global.binding.expect("checked texture binding");
            options.binding_map.insert(
                binding,
                hlsl::BindTarget {
                    register: slot as u32,
                    ..Default::default()
                },
            );
            sampled_bindings[slot] = !info.get_entry_point(0)[handle].is_empty();
        }
        let mut hlsl = String::new();
        let reflection = hlsl::Writer::new(&mut hlsl, &options, &Default::default())
            .write(&module, &info, None)
            .map_err(|error| CompileError::Invalid(error.to_string()))?;
        let entry = reflection
            .entry_point_names
            .first()
            .and_then(|name| name.as_ref().ok())
            .ok_or_else(|| CompileError::Invalid("Missing fragment entry point".into()))?;
        Ok(Self {
            bytecode: Self::compile_hlsl(&hlsl, entry)?,
            sampled_bindings,
            effect,
            buffers,
        })
    }

    fn compile_hlsl(source: &str, entry: &str) -> Result<Arc<[u8]>, CompileError> {
        let entry = CString::new(entry).map_err(|error| CompileError::Invalid(error.to_string()))?;
        let mut code = None;
        let mut diagnostics = None;
        // SAFETY: the source and null-terminated entry live through the call. The compiler may
        // populate the owned blobs. No include handler or native device participates.
        unsafe {
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("golden-fragment"),
                None,
                None,
                PCSTR(entry.as_ptr().cast()),
                s!("ps_5_0"),
                0,
                0,
                &mut code,
                Some(&mut diagnostics),
            )
        }
        .map_err(|error| {
            CompileError::Invalid(diagnostics.as_ref().map_or_else(
                || error.to_string(),
                |blob| {
                    String::from_utf8_lossy(blob_bytes(blob))
                        .trim_end_matches('\0')
                        .to_owned()
                },
            ))
        })?;
        let code = code
            .as_ref()
            .ok_or_else(|| CompileError::Invalid("Missing shader bytecode".into()))?;
        let code = blob_bytes(code);
        if code.is_empty() || code.len() > MAX_BYTECODE_BYTES {
            return Err(CompileError::Size);
        }
        Ok(Arc::from(code))
    }

    /// Which Media input bindings are used by the validated fragment entry point.
    pub fn sampled_inputs(&self) -> [bool; TEXTURE_INPUTS] {
        std::array::from_fn(|index| self.sampled_bindings[index])
    }

    /// Which Media and temporary buffer bindings are used by this program.
    pub fn sampled_bindings(&self) -> [bool; TEXTURE_BINDINGS] {
        self.sampled_bindings
    }

    pub(crate) fn effect(&self) -> uuid::Uuid {
        self.effect
    }

    pub(crate) fn raw_output(&self) -> bool {
        self.buffers.raw_output
    }

    pub(crate) fn raw_input(&self, slot: usize) -> bool {
        self.buffers.raw_inputs.get(slot).copied().unwrap_or(false)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.bytecode.len() + 2 * size_of::<usize>()
    }

    pub(crate) fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }
}

fn texture_slot(module: &naga::Module, global: &naga::GlobalVariable) -> Option<usize> {
    let ResourceBinding { group: 0, binding } = global.binding? else {
        return None;
    };
    let slot = binding.checked_sub(1)? as usize;
    if global.space != AddressSpace::Handle
        || global.name.as_deref()? != *adapter::TEXTURE_NAMES.get(slot)?
        || !matches!(
            module.types[global.ty].inner,
            TypeInner::Image {
                dim: naga::ImageDimension::D2,
                arrayed: false,
                class: naga::ImageClass::Sampled {
                    kind: naga::ScalarKind::Float,
                    multi: false
                },
            }
        )
    {
        return None;
    }
    Some(slot)
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    let size = unsafe { blob.GetBufferSize() };
    if size == 0 {
        return &[];
    }
    // SAFETY: an owned, non-empty compiler blob supplies that many initialized bytes.
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), size) }
}
