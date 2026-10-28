//! Disposable native feasibility harness. No product bootstrap or engine dependency.

#[cfg(all(windows, feature = "desktop"))]
mod cadence;
#[cfg(windows)]
mod card;
#[cfg(windows)]
mod child_resources;
#[cfg(windows)]
mod clock;
#[cfg(all(windows, feature = "desktop"))]
mod color_accuracy;
#[cfg(windows)]
mod completion;
#[cfg(windows)]
mod d3d;
#[cfg(all(windows, feature = "desktop"))]
mod desktop;
#[cfg(windows)]
mod dpi;
#[cfg(all(windows, feature = "desktop"))]
mod editor_isolation;
#[cfg(windows)]
mod etw_status;
#[cfg(windows)]
mod fixture;
#[cfg(all(windows, feature = "desktop"))]
mod frame_admission;
#[cfg(all(windows, feature = "desktop"))]
mod gpu_progress;
#[cfg(windows)]
mod observer;
#[cfg(windows)]
mod process;
#[cfg(windows)]
mod query_data;
#[cfg(windows)]
mod texture;
#[cfg(windows)]
mod trace_capability;
#[cfg(windows)]
mod wire;
#[cfg(all(windows, feature = "desktop"))]
use mapgyver_native_probe::presentation;
#[cfg(all(windows, feature = "desktop"))]
mod presenter;
#[cfg(all(windows, feature = "desktop"))]
mod quad;
#[cfg(all(windows, feature = "desktop"))]
mod render_worker;
#[cfg(all(windows, feature = "desktop"))]
mod resources;
#[cfg(all(windows, feature = "desktop"))]
mod retirement;
#[cfg(windows)]
mod smoke;
#[cfg(all(windows, feature = "desktop"))]
mod source_worker;
#[cfg(windows)]
mod spout;
#[cfg(all(windows, feature = "desktop"))]
mod timing;

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("native probe failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run() -> ProbeResult<()> {
    #[cfg(feature = "desktop")]
    use mapgyver_native_probe::workload::{RunSpec, Scenario};
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("trace-capability") if args.len() == 1 => print_json(&trace_capability::probe()?),
        Some("trace-status") if args.len() == 2 => print_json(&etw_status::query(&args[1])?),
        #[cfg(feature = "desktop")]
        Some("color-accuracy") if args.len() == 3 => {
            let report = color_accuracy::run(args[1].parse()?, args[2].parse()?)?;
            print_json(&report)?;
            if report.passed {
                Ok(())
            } else {
                Err("full-image accuracy diagnostic failed; see report".into())
            }
        }
        #[cfg(feature = "desktop")]
        Some("color-accuracy-processes") if args.len() == 3 => {
            let report = color_accuracy::processes::run(
                args[1].parse()?,
                args[2].parse()?,
                mapgyver_native_probe::color_reference::Fixture::Authored,
            )?;
            print_json(&report)?;
            if report.passed {
                Ok(())
            } else {
                Err("cross-process accuracy diagnostic failed; see report".into())
            }
        }
        #[cfg(feature = "desktop")]
        Some("color-procedural-accuracy") if args.len() == 3 => {
            let report = color_accuracy::processes::run(
                args[1].parse()?,
                args[2].parse()?,
                mapgyver_native_probe::color_reference::Fixture::Procedural,
            )?;
            print_json(&report)?;
            if report.passed {
                Ok(())
            } else {
                Err("procedural accuracy diagnostic failed; see report".into())
            }
        }
        #[cfg(feature = "desktop")]
        Some("color-child") if args.len() == 6 => color_accuracy::processes::child(
            args[1].parse()?,
            args[2].parse()?,
            args[3].parse()?,
            args[4].parse()?,
            &args[5],
        ),
        #[cfg(feature = "desktop")]
        Some("retirement") if args.len() == 4 => {
            let report = retirement::run(args[1].parse()?, args[2].parse()?, args[3].parse()?)?;
            print_json(&report)
        }
        #[cfg(feature = "desktop")]
        Some("gpu-progress") if args.len() == 3 => print_json(&gpu_progress::run(args[1].parse()?, args[2].parse()?)?),
        #[cfg(feature = "desktop")]
        Some("editor-isolation") if args.len() == 3 => print_json(&desktop::run(
            args[1].parse()?,
            RunSpec::editor_isolation(args[2].parse()?)?,
        )?),
        #[cfg(feature = "desktop")]
        Some("underlay") if args.len() == 3 => print_json(&desktop::run(
            args[1].parse()?,
            RunSpec::new(Scenario::Diagnostic, args[2].parse()?, 0)?,
        )?),
        #[cfg(feature = "desktop")]
        Some("recreate") if args.len() == 4 => print_json(&desktop::run(
            args[1].parse()?,
            RunSpec::recreation(args[2].parse()?, args[3].parse()?)?,
        )?),
        #[cfg(feature = "desktop")]
        Some("qualify") if args.len() == 5 => {
            let scenario = args[2].parse()?;
            if scenario == Scenario::Diagnostic {
                return Err("qualify requires q01 or q02".into());
            }
            print_json(&desktop::run(
                args[1].parse()?,
                RunSpec::new(scenario, args[3].parse()?, args[4].parse()?)?,
            )?)
        }
        Some("inventory") if args.len() == 1 => print_json(&d3d::inventory()?),
        Some("smoke") if args.len() == 2 => print_json(&smoke::run(args[1].parse()?)?),
        Some("observe") if args.len() == 4 => observer::run(args[1].parse()?, &args[2], args[3].parse()?),
        Some("fixture") if args.len() == 6 => {
            fixture::run_for(args[1].parse()?, &args[2], &args[3], args[4].parse()?, args[5].parse()?)
        }
        _ => Err(concat!(
            "usage: mapgyver_native_probe COMMAND\n",
            "  inventory\n",
            "  trace-capability (empty owned ETW session; no providers or files)\n",
            "  trace-status OWNED_PRESENTMON_SESSION (read-only live counters)\n",
            "  smoke ADAPTER\n",
            "  fixture ADAPTER NAME a|b FRAMES diagnostic|q01|q02\n",
            "  observe ADAPTER SENDER diagnostic|q01|q02\n",
            "desktop feature:\n",
            "  color-accuracy ADAPTER q01|q02 (finite authored cards; diagnostic readback, no timing claim)\n",
            "  color-accuracy-processes ADAPTER q01|q02 (producer/consumer children and source-worker bridge)\n",
            "  color-procedural-accuracy ADAPTER q01|q02 (actual procedural shader and full CPU reference)\n",
            "  underlay ADAPTER SECONDS\n",
            "  qualify ADAPTER q01|q02 SAMPLE_SECONDS WARMUP_SECONDS\n",
            "  recreate ADAPTER q01|q02 CYCLES\n",
            "  editor-isolation ADAPTER q01|q02 (busy, blank document, reconnect; 185s watchdog)\n",
            "  gpu-progress ADAPTER MODE (see native-probe README)\n",
            "  retirement ADAPTER CASE COUNT (cases: see README)"
        )
        .into()),
    }
}

#[cfg(not(windows))]
fn run() -> ProbeResult<()> {
    Err("native qualification requires x86_64-pc-windows-msvc".into())
}

#[cfg(windows)]
fn print_json(value: &impl serde::Serialize) -> ProbeResult<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
