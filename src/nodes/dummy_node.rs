use golden_core::{define_node_type, node::Node};

pub struct DummyNode {
    node_data: golden_core::node::NodeData,
    pub dummy_prop: String,
}

impl DummyNode {
    pub fn new(label: impl Into<String>, dummy_prop: String) -> Self {
        Self {
            node_data: golden_core::node::NodeData::new(label.into()),
            dummy_prop,
        }
    }

    pub fn print_dummy(&self) {
        println!("print dummy{}", self.dummy_prop);
    }
}

#[golden_core::node]
impl Node for DummyNode {
    fn init(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
        println!("DummyNode init: {}", self.dummy_prop);
    }

    fn destroy(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
        println!("DummyNode destroy: {}", self.dummy_prop);
    }
}


define_node_type!(
    pub struct DummyNode2 {
        pub dummy_prop: String,
    }
    type_name: "dummy2",
    node_impl {
        fn init(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
            println!("DummyNode2 init: {}", self.dummy_prop);
        }

        fn destroy(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
            println!("DummyNode2 destroy: {}", self.dummy_prop);
        }
    }
);
