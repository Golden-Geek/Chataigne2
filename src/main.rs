use golden_core::{node::Folder, run_app};

include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
pub type AppEngine = golden_core::engine::Engine<AppNode>;

fn main() -> std::io::Result<()> {
    let root: AppNode = Folder::new("Root".to_string()).into();
    let mut engine = AppEngine::new(root);

    // engine.add_node(DummyNode::new("Dum Dum").into(), None);
    engine.add_node(VeryDummyNode::create("Very Dummy").into(), None);
    // engine.add_node(SuperDummyNode::create("Super Dummy").into(), None);

    run_app(engine)
}
