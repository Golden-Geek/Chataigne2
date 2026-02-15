use golden_core::{
    node,
    node::{Node, NodeId, ParameterHandle},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
    update,
};

#[node]
#[params(
    feedback: f64 = 0.5 [0.0..1.0] (label = "Feedback");
    folder(output, label = "Output") {
        folder(color, label = "Color") {
            dummy_param: f64 = 2.2 [0.0..10.0] (label = "Dummy Param", description = "A parameter stored on a node using the #[param] attribute macro");
        }
        host: String = "127.0.0.1" (label = "Host");
    }
)]
pub struct DummyNode {}

#[node(from_struct)]
impl Node for DummyNode {
    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, node_id: NodeId, old_value: ParamValue) {
        if node_id == self.dummy_param.id() {
            println!("DummyNode dummy_param changed: old={:?} new={}", old_value, self.dummy_param.get());
        }
        println!("DummyNode parameter changed");
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        println!("DummyNode init, dummy_param initial value: {}", self.dummy_param.get());
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let osc: f64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64().sin() * 0.5 + 0.5;
        self.dummy_param.set(ctx, osc * 10.0);
        // println!("DummyNode update: {}", self.dummy_param.get());
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        println!("DummyNode destroy");
    }
}

#[node]
#[params(
folder(output, label = "Output", reuse = true) {
    very_dummy_param: f64 = 2.2 [0.0..10.0] (label = "Very Dummy Param", description = "A parameter stored on a node using the #[param] attribute macro");
}
)]
pub struct VeryDummyNode {
    dummy: DummyNode,
}

impl VeryDummyNode {
    pub fn create(label: impl Into<String>) -> Self {
        let label = label.into();
        Self::new(label.clone(), DummyNode::new(label))
    }
}

#[update(100)]
#[node(via = dummy, from_struct)]
impl Node for VeryDummyNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        println!("VeryDummyNode init, very_dummy_param initial value: {}", self.very_dummy_param.get());
        println!("VeryDummyNode init, dummy_param initial value: {}", self.dummy.dummy_param.get());
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.dummy.update(ctx);
    }

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, node_id: NodeId, old_value: ParamValue) {
        if node_id == self.very_dummy_param.id() {
            // println!("VeryDummyNode very_dummy_param changed: old={:?} new={}", old_value, self.very_dummy_param.get());
        }
        if node_id == self.dummy.dummy_param.id() {
            // println!("VeryDummyNode dummy_param changed: old={:?} new={}", old_value, self.dummy.dummy_param.get());
        }
    }
}

#[node]
pub struct SuperDummyNode {
    very_dummy: VeryDummyNode,

    #[param(default = 0.25, min = 0.0, max = 1.0, label = "Super Dummy Param", description = "A parameter stored on a node using via composition")]
    super_dummy_param: ParameterHandle<f64>,
}

impl SuperDummyNode {
    pub fn create(label: impl Into<String>) -> Self {
        let label = label.into();
        Self::new(label.clone(), VeryDummyNode::create(label))
    }
}

#[node(via = very_dummy, from_struct)]
impl Node for SuperDummyNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        println!("SuperDummyNode init, super_dummy_param initial value: {}", self.super_dummy_param.get());
        println!("SuperDummyNode init, very_dummy_param initial value: {}", self.very_dummy.very_dummy_param.get());
    }

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, node_id: NodeId, old_value: ParamValue) {
        if node_id == self.super_dummy_param.id() {
            // println!("SuperDummyNode super_dummy_param changed: old={:?} new={}", old_value, self.super_dummy_param.get());
        }
        if node_id == self.very_dummy.very_dummy_param.id() {
            // println!("SuperDummyNode very_dummy_param changed: old={:?} new={}", old_value, self.very_dummy.very_dummy_param.get());
        }
    }
}
