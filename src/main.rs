include!(concat!(env!("OUT_DIR"), "/app_nodes.rs"));
pub type AppEngine = golden_core::engine::Engine<AppNode>;
use golden_core::node::Container;

fn main() {
    let root: AppNode = Container::new("Root".to_string()).into();
    let mut engine = AppEngine::new(root);

    let node = DummyNode::new("Dummy", "Some dummy prop".to_string());
    engine.add_node(node.into(), None);
    engine.add_node(DummyNode2::new("Dummy2", "Another dummy prop".to_string()).into(), None);
    engine.add_node(SuperDummy::new("SuperDummy".to_string(), "Some dummy prop".to_string(), "Some superdummy prop".to_string()).into(), None);
    engine.add_node(VeryDummy::new("VeryDummy", "Some dummy prop".to_string(), "Some verydummy prop".to_string()).into(), None);
}
