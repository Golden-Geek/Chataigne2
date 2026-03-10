mod bootstrap;
mod desktop;
mod ui_server;

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));

pub type AppEngine = golden_core::engine::Engine<AppNode>;

pub fn run() -> std::io::Result<()> {
    desktop::run_app::<AppNode>()
}
