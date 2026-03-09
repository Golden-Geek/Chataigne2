mod bootstrap;

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));

pub type AppEngine = golden_core::engine::Engine<AppNode>;

pub fn run() -> std::io::Result<()> {
    let engine = bootstrap::build_engine();
    golden_core::run_app(engine)
}
