//! Owned qualification roles. Production control stays metadata-only; desktop-capture exports
//! a bounded owned pixel strip to its independent test collector.
#[cfg(windows)]
mod config;
#[cfg(windows)]
mod desktop_capture;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod lifecycle;
#[cfg(windows)]
mod observations;
#[cfg(windows)]
mod preferences;
#[cfg(windows)]
mod roles;
#[cfg(windows)]
mod scene;

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    let result = (|| {
        let mut args = config::Arguments::read()?;
        match args.take("role", "renderer").as_str() {
            "renderer" => roles::renderer(args),
            "lifecycle" => lifecycle::run(args),
            "source" => roles::source(args),
            "consumer" => roles::consumer(args),
            "catalog" => observations::catalog(args),
            "hardware" => observations::hardware(args),
            "pixels" => observations::pixels(args),
            "calibration-pixels" => observations::calibration_pixels(args),
            "surround-pixels" => observations::surround_pixels(args),
            "gpu-preferences" => preferences::write(args),
            "desktop-capture" => desktop_capture::run(args),
            _ => Err(std::io::Error::other("unknown qualification role")),
        }
    })();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("native qualification failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("native qualification requires Windows");
    std::process::exit(1);
}
