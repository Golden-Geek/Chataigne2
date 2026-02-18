use golden_core::{
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
    update,
};

#[node]
#[params(
    feedback: f64 = 0.5 [0.0..1.0] (label = "Feedback");
    folder(output, label = "Output 2") {
        host: String = "127.0.0.1" (label = "Host");
        folder(color, label = "Color") {
            dummy_param: f64 = 2.2 [0.0..10.0] (label = "Dummy Param", description = "A parameter stored on a node using the #[param] attribute macro");
        }
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


// VERY DUMMY

#[node] //est-ce que celui la crée quand meme un node_data dans VeryDummyNode ?
#[params(
folder(output, label = "Output", reuse = true) {
    very_dummy_param: f64 = 2.2 [0.0..10.0] (label = "Very Dummy Param", description = "A parameter stored on a node using the #[param] attribute macro", default_callback);
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

    fn on_very_dummy_param_change(&mut self, _ctx: &mut ProcessCtx, _old_value: ParamValue) {
        if cfg!(debug_assertions) {
            // println!(
                // "VeryDummyNode very_dummy_param changed: old={old_value:?} new={}",
                // self.very_dummy_param.get()
            // );
        }
    }
}

#[update(1)]
#[node(via = dummy, from_struct)]
impl Node for VeryDummyNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        println!("VeryDummyNode init, very_dummy_param initial value: {}", self.very_dummy_param.get());
        println!("VeryDummyNode init, dummy_param initial value: {}", self.dummy.dummy_param.get());
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.dummy.update(ctx);
    }

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, node_id: NodeId, _old_value: ParamValue) {
        if node_id == self.dummy.dummy_param.id() {
            if cfg!(debug_assertions) {
                // println!(
                //     "VeryDummyNode dummy_param changed: old={old_value:?} new={}",
                //     self.dummy.dummy_param.get()
                // );
            }
        }
    }
}
