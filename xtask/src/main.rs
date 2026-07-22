use std::process::ExitCode;

fn main() -> ExitCode {
    match xtask::run(std::env::args().skip(1)) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("[watch][error] {error}");
            xtask::output::error(&error);
            ExitCode::from(2)
        }
    }
}
