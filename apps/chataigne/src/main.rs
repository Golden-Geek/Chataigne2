#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod product_evidence;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::support as test_support;

fn main() -> std::io::Result<()> {
    app::run()
}
