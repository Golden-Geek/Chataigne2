fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: generate_preview_protocol <output-directory>")?;
    mapgyver::preview::protocol::export_typescript(std::path::PathBuf::from(directory))
}
