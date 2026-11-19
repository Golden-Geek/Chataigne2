use std::ffi::CString;
use std::fmt;
use std::sync::Arc;

use naga::{AddressSpace, Binding, BuiltIn, ResourceBinding, ShaderStage, TypeInner, back::hlsl, front::glsl, valid};
use windows::Win32::Graphics::Direct3D::{Fxc::D3DCompile, ID3DBlob};
use windows::core::{PCSTR, s};

use super::MAX_SOURCE_BYTES;

const MAX_BYTECODE_BYTES: usize = 256 * 1024;
const FRAME_BINDING: ResourceBinding = ResourceBinding { group: 0, binding: 0 };

const INPUTS: &str = r#"
layout(std140, binding = 0) uniform GoldenFrame { vec4 sizeAndTime; } goldenFrame;
#define iResolution (goldenFrame.sizeAndTime.xyz)
#define iTime (goldenFrame.sizeAndTime.w)
layout(location = 0) out vec4 fragColor;
#define main goldenUserMain
"#;

const ENTRY: &str = r#"
#undef main
void main() {
    fragColor = vec4(0.0);
    goldenUserMain();
    fragColor.a = clamp(fragColor.a, 0.0, 1.0);
    fragColor.rgb *= fragColor.a;
}
"#;

/// A validated device-independent fragment program. Cloning shares its bytecode.
#[derive(Clone)]
pub struct FragmentProgram(Arc<[u8]>);

impl fmt::Debug for FragmentProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FragmentProgram")
            .field("bytecode_bytes", &self.0.len())
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
    /// `gl_FragCoord` follows Vulkan's upper-left origin. No other stage inputs or
    /// resource bindings are furnished by this entry point.
    pub fn compile_glsl(source: &str) -> Result<Self, CompileError> {
        if source.trim().is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err(CompileError::Size);
        }
        let source = source.trim_start_matches('\u{feff}').trim_start();
        let source = if source.starts_with("#version ") {
            source.split_once('\n').map_or("", |(_, body)| body)
        } else {
            source
        };
        let glsl = format!("#version 450 core\n{INPUTS}\n{source}\n{ENTRY}");
        let module = glsl::Frontend::default()
            .parse(&glsl::Options::from(ShaderStage::Fragment), &glsl)
            .map_err(|error| CompileError::Invalid(error.emit_to_string(&glsl)))?;
        if module.global_variables.iter().any(|(_, global)| {
            !(global.space == AddressSpace::Private
                || (global.space == AddressSpace::Uniform && global.binding == Some(FRAME_BINDING)))
        }) {
            return Err(CompileError::Invalid("Unsupported GLSL resource binding".into()));
        }
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
            .map_err(|error| CompileError::Invalid(error.emit_to_string(&glsl)))?;
        let mut options = hlsl::Options {
            shader_model: hlsl::ShaderModel::V5_0,
            fake_missing_bindings: false,
            ..Default::default()
        };
        options.binding_map.insert(FRAME_BINDING, hlsl::BindTarget::default());
        let mut hlsl = String::new();
        let reflection = hlsl::Writer::new(&mut hlsl, &options, &Default::default())
            .write(&module, &info, None)
            .map_err(|error| CompileError::Invalid(error.to_string()))?;
        let entry = reflection
            .entry_point_names
            .first()
            .and_then(|name| name.as_ref().ok())
            .ok_or_else(|| CompileError::Invalid("Missing fragment entry point".into()))?;
        Self::compile_hlsl(&hlsl, entry)
    }

    fn compile_hlsl(source: &str, entry: &str) -> Result<Self, CompileError> {
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
        Ok(Self(Arc::from(code)))
    }

    pub(crate) fn bytecode(&self) -> &[u8] {
        &self.0
    }
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    let size = unsafe { blob.GetBufferSize() };
    if size == 0 {
        return &[];
    }
    // SAFETY: an owned, non-empty compiler blob supplies that many initialized bytes.
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), size) }
}
