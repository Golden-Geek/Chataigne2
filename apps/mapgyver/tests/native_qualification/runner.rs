//! Owned production-runtime qualification roles. No frame pixels enter control IPC.
#[cfg(windows)]
mod config;
#[cfg(windows)]
mod host;
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
            "source" => roles::source(args),
            "consumer" => roles::consumer(args),
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
