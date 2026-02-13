use golden_core::{
    define_node_enum, engine::Engine, node::Container
};

define_node_enum!(enum ChataigneNode {
    // App-specific node types go here.
});

fn main() {
    let root: ChataigneNode = Container::new("Root".to_string()).into();
    let _engine = Engine::<ChataigneNode>::new(root);
}
