#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> std::io::Result<()> {
    mapgyver::app::run()
}
