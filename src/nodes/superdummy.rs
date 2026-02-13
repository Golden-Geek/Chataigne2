use crate::DummyNode;
use golden_core::{define_node_type, node::Node};

pub struct SuperDummy {
    dummy_node: DummyNode,
    superdummy_prop: String,
}

impl SuperDummy {
    pub fn new(label: String, dummy_prop: String, superdummy_prop: String) -> Self {
        Self {
            dummy_node: DummyNode::new(label, dummy_prop),
            superdummy_prop,
        }
    }

    pub fn print_superdummy(&self) {
        self.dummy_node.print_dummy();
        println!("print superdummy {}, {}", self.dummy_node.dummy_prop, self.superdummy_prop);
    }
}

#[golden_core::node(via = dummy_node)]
impl Node for SuperDummy {
    fn init(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
        println!("SuperDummy init: {}", self.superdummy_prop);
    }

    fn destroy(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
        println!("SuperDummy destroy: {}", self.superdummy_prop);
    }
}

define_node_type!(
    pub struct VeryDummy {
        pub dummy_node: DummyNode,
        pub verydummy_prop: String,
    }
    type_name: "verydummy",
    via: dummy_node,
    constructor(label, dummy_prop: String, verydummy_prop: String) {
        Self {
            dummy_node: DummyNode::new(label, dummy_prop),
            verydummy_prop,
        }
    },
    node_impl {
        fn init(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
            println!("VeryDummy init: {}", self.verydummy_prop);
        }

        fn destroy(&mut self, _ctx: &mut golden_core::process_ctx::ProcessCtx) {
            println!("VeryDummy destroy: {}", self.verydummy_prop);
        }
    }
);


