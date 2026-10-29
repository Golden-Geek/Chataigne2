use golden_video::runtime::{DeviceGeneration, ProjectEpoch, VideoRuntime};
use golden_video::{TextureDescriptor, TextureId};

#[derive(Clone, Debug)]
pub struct SourceObservations {
    pub(super) project: ProjectEpoch,
    pub(super) device: Option<DeviceGeneration>,
    pub(super) sources: Vec<Source>,
}

#[derive(Clone, Debug)]
pub(super) struct Source {
    pub guard: golden_io::RevisionGuard,
    pub texture: TextureId,
    pub binding: uuid::Uuid,
    pub generation: u64,
    pub pool_generation: u64,
    pub descriptor: Option<TextureDescriptor>,
    pub incompatible: bool,
}

impl SourceObservations {
    pub(super) fn equivalent(&self, other: &Self) -> bool {
        self.project == other.project
            && self.device == other.device
            && self.sources.len() == other.sources.len()
            && self.sources.iter().zip(&other.sources).all(|(a, b)| {
                a.guard.is_current()
                    && b.guard.is_current()
                    && (
                        a.texture,
                        a.binding,
                        a.generation,
                        a.pool_generation,
                        a.descriptor,
                        a.incompatible,
                    ) == (
                        b.texture,
                        b.binding,
                        b.generation,
                        b.pool_generation,
                        b.descriptor,
                        b.incompatible,
                    )
            })
    }
}

pub(super) fn capture(runtime: &VideoRuntime) -> SourceObservations {
    let snapshot = runtime.snapshot();
    let device = snapshot.resources.map(|resources| resources.last_submitted.device);
    #[cfg(not(all(windows, feature = "native-video")))]
    let sources = Vec::new();
    #[cfg(all(windows, feature = "native-video"))]
    let sources = runtime
        .d3d11_control()
        .map(|control| {
            use golden_video::native::SourceState;
            control
                .source_observations()
                .iter()
                .filter(|source| Some(source.snapshot.device) == device)
                .map(|observed| {
                    let source = &observed.snapshot;
                    Source {
                        guard: observed.guard.clone(),
                        texture: source.texture,
                        binding: source.binding,
                        generation: source.generation,
                        pool_generation: source.pool_generation,
                        descriptor: (source.state == SourceState::Streaming)
                            .then_some(source.observed_descriptor)
                            .flatten(),
                        incompatible: source.state == SourceState::DescriptorMismatch,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    SourceObservations {
        project: snapshot.project,
        device,
        sources,
    }
}
