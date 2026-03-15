#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;

fn main() -> std::io::Result<()> {
    app::run()
}
