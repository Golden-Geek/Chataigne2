fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("expected generated output directory")?;
    golden_video::qualification::protocol::export_typescript(directory)
}
