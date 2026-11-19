fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: generate_file_explorer_types <output-directory>")?;
    golden_host_desktop::export_file_explorer_typescript(directory.into())
}
