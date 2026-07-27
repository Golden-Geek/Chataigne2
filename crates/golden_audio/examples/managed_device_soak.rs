use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use golden_audio::{
    GainDb, SampleRate, compiled_cpal_backends,
    qualification::{
        ManagedDeviceSoakOptions, ReferenceWorkload, run_managed_device_soak_with_progress, write_reference_wave,
    },
};

#[derive(Debug)]
struct Arguments {
    backend: String,
    duration: Duration,
    recovery_interval: Option<Duration>,
    poll_interval: Duration,
    readiness_timeout: Duration,
    workload: ReferenceWorkload,
    playback_gain: GainDb,
    revision: String,
    report_path: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run() {
        Ok(passed) if passed => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(2),
        Err(error) => {
            eprintln!("managed device soak failed to run: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let Some(arguments) = parse_arguments(env::args().skip(1))? else {
        print_help();
        return Ok(true);
    };
    let backend = compiled_cpal_backends()
        .into_iter()
        .find(|backend| backend.id().as_str() == arguments.backend)
        .ok_or_else(|| format!("backend {:?} is not compiled", arguments.backend))?;
    let fixture = TemporaryFixture::new()?;
    write_reference_wave(&fixture.path, Duration::from_secs(5), SampleRate::default())?;
    let options = ManagedDeviceSoakOptions {
        duration: arguments.duration,
        recovery_interval: arguments.recovery_interval,
        poll_interval: arguments.poll_interval,
        readiness_timeout: arguments.readiness_timeout,
        workload: arguments.workload,
        playback_gain: arguments.playback_gain,
        revision: arguments.revision,
        ..ManagedDeviceSoakOptions::default()
    };
    let mut last_heartbeat = Duration::ZERO;
    let mut last_recovery_count = 0_u32;
    let report = run_managed_device_soak_with_progress(backend, options, &fixture.path, |progress| {
        let elapsed = Duration::from_millis(progress.elapsed_ms);
        if elapsed.saturating_sub(last_heartbeat) >= Duration::from_secs(30)
            || progress.completed_recovery_cycles != last_recovery_count
        {
            eprintln!(
                "elapsed={}s readiness={:?} frames={} voices={} xruns={} recoveries={}",
                elapsed.as_secs(),
                progress.readiness,
                progress.rendered_frames,
                progress.active_voices,
                progress.xrun_count,
                progress.completed_recovery_cycles
            );
            last_heartbeat = elapsed;
            last_recovery_count = progress.completed_recovery_cycles;
        }
    })?;
    let json = serde_json::to_string_pretty(&report)?;
    println!("{json}");
    if let Some(path) = arguments.report_path {
        fs::write(&path, format!("{json}\n"))
            .map_err(|error| format!("failed to write report {}: {error}", path.display()))?;
    }
    Ok(report.passed)
}

fn parse_arguments(
    mut arguments: impl Iterator<Item = String>,
) -> Result<Option<Arguments>, Box<dyn std::error::Error>> {
    let mut parsed = Arguments {
        backend: native_backend_id().to_owned(),
        duration: Duration::from_secs(60 * 60),
        recovery_interval: Some(Duration::from_secs(10 * 60)),
        poll_interval: Duration::from_millis(250),
        readiness_timeout: Duration::from_secs(30),
        workload: ReferenceWorkload::Medium,
        playback_gain: GainDb::new(-96.0)?,
        revision: revision_label(),
        report_path: None,
    };
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
            "--backend" => parsed.backend = next_value(&mut arguments, "--backend")?,
            "--seconds" => {
                parsed.duration = Duration::from_secs(parse_value(&mut arguments, "--seconds")?);
            }
            "--recovery-seconds" => {
                let seconds = parse_value(&mut arguments, "--recovery-seconds")?;
                parsed.recovery_interval = (seconds > 0).then(|| Duration::from_secs(seconds));
            }
            "--poll-ms" => {
                parsed.poll_interval = Duration::from_millis(parse_value(&mut arguments, "--poll-ms")?);
            }
            "--readiness-seconds" => {
                parsed.readiness_timeout = Duration::from_secs(parse_value(&mut arguments, "--readiness-seconds")?);
            }
            "--workload" => {
                parsed.workload = parse_workload(next_value(&mut arguments, "--workload")?.as_str())?;
            }
            "--playback-gain-db" => {
                parsed.playback_gain = GainDb::new(parse_value(&mut arguments, "--playback-gain-db")?)?;
            }
            "--revision" => parsed.revision = next_value(&mut arguments, "--revision")?,
            "--report" => {
                parsed.report_path = Some(PathBuf::from(next_value(&mut arguments, "--report")?));
            }
            _ => return Err(format!("unknown argument {argument:?}; use --help for usage").into()),
        }
    }
    Ok(Some(parsed))
}

fn next_value(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, Box<dyn std::error::Error>> {
    arguments
        .next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn parse_value<T>(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + 'static,
{
    let value = next_value(arguments, flag)?;
    value
        .parse::<T>()
        .map_err(|error| format!("invalid {flag} value {value:?}: {error}").into())
}

fn parse_workload(value: &str) -> Result<ReferenceWorkload, Box<dyn std::error::Error>> {
    match value {
        "small" => Ok(ReferenceWorkload::Small),
        "medium" => Ok(ReferenceWorkload::Medium),
        "large" => Ok(ReferenceWorkload::Large),
        "extreme-offline" => Ok(ReferenceWorkload::ExtremeOffline),
        _ => Err(format!("unknown workload {value:?}; expected small, medium, or large").into()),
    }
}

fn revision_label() -> String {
    let revision = git_output(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    let dirty = git_output(&["status", "--porcelain"]).is_some_and(|status| !status.is_empty());
    if dirty { format!("{revision}+dirty") } else { revision }
}

fn git_output(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn native_backend_id() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "wasapi"
    }
    #[cfg(target_os = "macos")]
    {
        "coreaudio"
    }
    #[cfg(target_os = "linux")]
    {
        "alsa"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "cpal-null"
    }
}

fn print_help() {
    println!(
        "Usage: managed_device_soak [options]\n\
         \n\
         Runs the managed Golden Audio callback path against the system default output.\n\
         Defaults: 3600 seconds, medium workload, recovery cycle every 600 seconds.\n\
         \n\
         Options:\n\
           --backend ID              Native backend ID\n\
           --seconds N               Total run duration\n\
           --recovery-seconds N      Disable/ready cycle interval; 0 disables cycles\n\
           --poll-ms N               Observation interval\n\
           --readiness-seconds N     Device readiness timeout\n\
           --workload NAME           small, medium, or large\n\
           --playback-gain-db DB     Per-voice fixture gain (default -96 dB)\n\
           --revision LABEL          Exact revision label for the report\n\
           --report PATH             Also write pretty JSON to PATH\n\
           -h, --help                Show this help"
    );
}

#[derive(Debug)]
struct TemporaryFixture {
    path: PathBuf,
}

impl TemporaryFixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        Ok(Self {
            path: env::temp_dir().join(format!("golden-audio-managed-soak-{}-{unique}.wav", std::process::id())),
        })
    }
}

impl Drop for TemporaryFixture {
    fn drop(&mut self) {
        let _ = remove_exact_file(&self.path);
    }
}

fn remove_exact_file(path: &Path) -> std::io::Result<()> {
    if path.is_file() {
        fs::remove_file(path)?;
    }
    Ok(())
}
