use golden_core::define_node_type;

define_node_type!(
    pub struct DummyNode {
        pub dummy_prop: String,
    }
    type_name: "dummy"
);

impl DummyNode {
    pub fn print_dummy(&self) {
        println!("print dummy{}", self.dummy_prop);
    }
}
