mod cli;
pub mod output;
mod process;
pub mod readiness;
mod watch;

#[cfg(test)]
mod watch_tests;

pub use cli::{ParseOutcome, WatchConfig, usage};

pub fn run<I, S>(args: I) -> Result<u8, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    match WatchConfig::parse(args)? {
        ParseOutcome::Help => {
            print!("{}", usage());
            Ok(0)
        }
        ParseOutcome::Watch(config) => watch::run(config),
    }
}
