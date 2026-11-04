//! Shared preview admission includes image leases bounded by the preparation cache.

use golden_assets::PreparationLimits;
use golden_io::WorkerNotifier;
use golden_video::{
    plan::PlanLimits,
    runtime::{ResourceLimits, RuntimeConfig},
};
use std::{num::NonZeroUsize, time::Duration};

/// Logical dependency metadata, including References and procedural channels. Capturing a
/// definition never admits a native producer; rendering applies its independent graph limits.
pub const MAX_CAPTURE_CHANNELS: usize = 128;
/// Existing physical input/texture-binding ceiling shared by the plan and native receiver.
pub const MAX_NATIVE_PRODUCERS: usize = 8;

/// Product bounds shared by the desktop host and explicit native qualification callers.
pub struct Config {
    pub video: RuntimeConfig,
    pub pictures: PreparationLimits,
}

/// The caller obtains `admission_limit` from Golden's measured LOCAL-adapter budget before
/// creating native resources. Logical plan limits never override whole-runtime accounting.
pub fn configuration(admission_limit: u64, notifier: Option<WorkerNotifier>) -> Config {
    let pictures = PreparationLimits {
        max_source_bytes: 64 * 1024 * 1024,
        max_decode_bytes: NonZeroUsize::new(128 * 1024 * 1024).unwrap(),
        max_prepared_bytes: NonZeroUsize::new(256 * 1024 * 1024).unwrap(),
        ..Default::default()
    };
    // Raw plan accounting includes retained image storage, not only graph metadata.
    // These charges do not allocate another pixel copy or expand the cache's physical ceiling.
    let cache = pictures.max_prepared_bytes.get();
    Config {
        video: RuntimeConfig {
            plan_limits: PlanLimits {
                max_passes: 24,
                max_external_textures: MAX_NATIVE_PRODUCERS,
                max_outputs: crate::screens::rendering::MAX_SCREEN_OUTPUTS + 1,
                max_dependencies: 128,
                max_texture_dimension: 8192,
                max_texture_bytes: admission_limit,
            },
            resource_limits: ResourceLimits {
                max_bytes: admission_limit,
                max_in_flight: NonZeroUsize::new(2).unwrap(),
                max_retiring: 2,
            },
            frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
            max_outstanding: NonZeroUsize::new(8).unwrap(),
            max_input_bytes: NonZeroUsize::new(cache + 1024 * 1024).unwrap(),
            max_retained_input_bytes: NonZeroUsize::new(cache + 4 * 1024 * 1024).unwrap(),
            recovery_delay: Duration::from_millis(100),
            max_recovery_delay: Duration::from_secs(2),
            notifier,
        },
        pictures,
    }
}
