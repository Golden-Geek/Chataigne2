use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: generate_state_machine_protocol <output-directory>")?;
    chataigne_state_machine::export_typescript(output)?;
    Ok(())
}
