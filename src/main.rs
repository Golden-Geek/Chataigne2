include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
pub type AppEngine = golden_core::engine::Engine<AppNode>;
use golden_core::{node::{Container}};

fn main() {
    let root: AppNode = Container::new("Root".to_string()).into();
    let mut engine = AppEngine::new(root);

    let node = DummyNode::new("Dummy", "Some dummy prop".to_string());
    engine.add_node(node.into(), None);
}
