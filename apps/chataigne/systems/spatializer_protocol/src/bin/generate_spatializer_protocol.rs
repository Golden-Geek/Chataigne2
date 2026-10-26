fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: generate_spatializer_protocol <output-directory>")?;
    chataigne_spatializer_protocol::export_typescript(std::path::PathBuf::from(directory))
}
