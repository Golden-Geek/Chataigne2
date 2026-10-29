fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: generate_media_protocol <output-directory>")?;
    mapgyver::media::protocol::export_typescript(std::path::PathBuf::from(directory))
}
