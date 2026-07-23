use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: generate_golden_audio_contract <output-directory>")?;
    golden_audio::contract::export_device_contract(output)
}
