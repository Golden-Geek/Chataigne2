mod buffers;
mod clock;
mod compiler;
mod convert;
mod gain;
mod offline;
mod plan;
mod processor;
mod reference;
mod route_kernel;

pub use buffers::PlanarBuffer;
pub use clock::OfflineClock;
pub use compiler::{RenderCompileContext, RenderPlanCompilation, RenderPlanCompiler};
pub use convert::{ConversionStats, InterleavedInput, InterleavedOutput, deinterleave, interleave};
pub use gain::GainSmoother;
pub use offline::OfflineRenderer;
pub use plan::{
    CompiledAnalysisTap, CompiledRoute, CompiledRouteMatrix, RenderPlan, RenderWarning, RenderWarningCode, RouteSpan,
};
pub use processor::{RenderProcessor, RenderProcessorMetrics};
pub use reference::render_scalar_reference;

#[cfg(test)]
mod tests;
