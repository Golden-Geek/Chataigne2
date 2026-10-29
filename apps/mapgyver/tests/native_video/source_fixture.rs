//! Owned independent-process Spout source for finite native input verification.
#[cfg(windows)]
mod fixture {
    use std::io::{self, BufRead, Error, Write};
    use std::num::NonZeroUsize;
    use std::time::{Duration, Instant};

    use golden_time::RationalRate;
    use golden_video::native::{AdapterId, D3d11Config, OutputBinding, adapters};
    use golden_video::plan::{PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan};
    use golden_video::runtime::{LatePolicy, PlanActivation, ResourceLimits, RuntimeConfig, VideoRuntime};
    use golden_video::{
        AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
    };

    struct Running(VideoRuntime);
    impl Drop for Running {
        fn drop(&mut self) {
            self.0.close();
            while !self.0.retirement().wait(Duration::from_millis(20)) {}
        }
    }

    fn config() -> RuntimeConfig {
        RuntimeConfig {
            plan_limits: PlanLimits {
                max_passes: 4,
                max_external_textures: 0,
                max_outputs: 1,
                max_dependencies: 4,
                max_texture_dimension: 1024,
                max_texture_bytes: 16 * 1024 * 1024,
            },
            resource_limits: ResourceLimits {
                max_bytes: 64 * 1024 * 1024,
                max_in_flight: NonZeroUsize::new(2).unwrap(),
                max_retiring: 2,
            },
            frame_rate: RationalRate::new(60, 1).unwrap(),
            max_outstanding: NonZeroUsize::new(4).unwrap(),
            max_input_bytes: NonZeroUsize::new(65536).unwrap(),
            max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
            recovery_delay: Duration::from_millis(100),
            max_recovery_delay: Duration::from_secs(1),
            notifier: None,
        }
    }

    fn start(adapter: AdapterId, name: &str, width: u32, height: u32, color: [f32; 3]) -> io::Result<Running> {
        let descriptor = TextureDescriptor {
            extent: Extent2D::new(width, height).map_err(Error::other)?,
            format: PixelFormat::Bgra8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Opaque,
        };
        let mut runtime = VideoRuntime::spawn_d3d11(
            config(),
            D3d11Config {
                adapter,
                inputs: Default::default(),
                outputs: vec![OutputBinding::Spout {
                    id: OutputId::new(1).unwrap(),
                    name: name.into(),
                    descriptor,
                }],
            },
        )
        .map_err(Error::other)?;
        let project = runtime.project_epoch();
        let epoch = runtime.read_time().map_err(Error::other)?.epoch();
        let plan = RenderPlan {
            external_textures: vec![],
            passes: vec![RenderPass {
                id: PassId::new(1).unwrap(),
                output: TextureId::new(1).unwrap(),
                descriptor: TextureDescriptor {
                    color_encoding: ColorEncoding::Linear,
                    alpha_mode: AlphaMode::Premultiplied,
                    ..descriptor
                },
                operation: RenderOperation::Clear(
                    LinearColor::from_straight(color[0], color[1], color[2], 1.0).map_err(Error::other)?,
                ),
            }],
            outputs: vec![PlanOutput {
                id: OutputId::new(1).unwrap(),
                source: TextureId::new(1).unwrap(),
            }],
        };
        runtime
            .submit_plan(project, plan, PlanActivation::NextFrame(epoch), LatePolicy::Activate)
            .map_err(Error::other)?;
        let running = Running(runtime);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = running.0.snapshot();
            if let Some(failure) = snapshot.failure {
                return Err(Error::other(failure.to_string()));
            }
            if running.0.d3d11_control().unwrap().snapshot().accepted_outputs >= 2 {
                return Ok(running);
            }
            if Instant::now() >= deadline {
                return Err(Error::other("source fixture startup timed out"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn run() -> io::Result<()> {
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        if args.len() != 4 || args[0] != "--adapter" || args[2] != "--sender" {
            return Err(Error::other("expected --adapter <index> --sender <owned-name>"));
        }
        let index: usize = args[1].parse().map_err(Error::other)?;
        let inventory = adapters().map_err(Error::other)?;
        let adapter = inventory
            .get(index)
            .filter(|adapter| !adapter.software)
            .ok_or_else(|| Error::other("selected hardware adapter absent"))?
            .id;
        let mut running: Option<Running> = None;
        println!("OK ready");
        io::stdout().flush()?;
        for line in io::stdin().lock().lines() {
            let line = line?;
            let fields = line.split_whitespace().collect::<Vec<_>>();
            match fields.as_slice() {
                ["start", width, height, red, green, blue] => {
                    drop(running.take());
                    running = Some(start(
                        adapter,
                        &args[3],
                        width.parse().map_err(Error::other)?,
                        height.parse().map_err(Error::other)?,
                        [
                            red.parse().map_err(Error::other)?,
                            green.parse().map_err(Error::other)?,
                            blue.parse().map_err(Error::other)?,
                        ],
                    )?);
                    println!("OK started");
                }
                ["stop"] => {
                    drop(running.take());
                    println!("OK stopped");
                }
                ["pause"] => {
                    let runtime = &mut running.as_mut().ok_or_else(|| Error::other("no running source"))?.0;
                    runtime
                        .submit_plan(
                            runtime.project_epoch(),
                            RenderPlan::default(),
                            PlanActivation::NextFrame(runtime.read_time().map_err(Error::other)?.epoch()),
                            LatePolicy::Activate,
                        )
                        .map_err(Error::other)?;
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while !runtime
                        .snapshot()
                        .resources
                        .is_some_and(|resources| resources.active.is_some_and(|active| active.output_count == 0))
                    {
                        if Instant::now() > deadline {
                            return Err(Error::other("source pause timed out"));
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    println!("OK paused");
                }
                ["exit"] => {
                    drop(running.take());
                    println!("OK exit");
                    io::stdout().flush()?;
                    break;
                }
                _ => return Err(Error::other("invalid fixture command")),
            }
            io::stdout().flush()?;
        }
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match fixture::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("native source fixture failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("native source fixture requires Windows");
}
