#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod product_evidence;
#[cfg(test)]
mod test_support;

fn main() -> std::io::Result<()> {
    app::run()
}
