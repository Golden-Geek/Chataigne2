use golden_core::{
    item, node,
    node::{Node, NodeUserPermissions},
    process_ctx::ProcessCtx,
};

/// Legacy filter wrapper retained for tree/UI persistence.
///
/// Runtime remapping behavior belongs to the Alchemist `remap` ANode and the
/// managed filter-pipeline lowering path.
#[node("sm_remap_filter", label = "Range Remap")]
#[children(
    in_min: f64 = 0.0 (
        label = "In Min"
    );
    in_max: f64 = 1.0 (
        label = "In Max"
    );
    out_min: f64 = 0.0 (
        label = "Out Min"
    );
    out_max: f64 = 1.0 (
        label = "Out Max"
    );
)]
pub struct RangeRemapFilter {}

#[item("sm_filter", node = "sm_remap_filter", from_struct)]
impl Node for RangeRemapFilter {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

/// Legacy filter wrapper retained for tree/UI persistence.
///
/// Runtime smoothing behavior belongs to the Alchemist `smooth_filter` ANode
/// and the managed filter-pipeline lowering path.
#[node("sm_smoothing_filter", label = "Smoothing")]
#[children(
    factor: f64 = 0.1 [0.0..1.0] (
        label = "Factor"
    );
)]
pub struct SmoothingFilter {}

#[item("sm_filter", node = "sm_smoothing_filter", from_struct)]
impl Node for SmoothingFilter {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

/// Legacy filter wrapper retained for tree/UI persistence.
///
/// Runtime inversion behavior belongs to the Alchemist `one_minus` ANode and
/// the managed filter-pipeline lowering path.
#[node("sm_invert_filter", label = "Invert")]
pub struct InvertFilter {}

#[item("sm_filter", node = "sm_invert_filter", from_struct)]
impl Node for InvertFilter {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

/// Legacy filter wrapper retained for tree/UI persistence.
///
/// Runtime clamping behavior belongs to the Alchemist `clamp` ANode and the
/// managed filter-pipeline lowering path.
#[node("sm_clamp_filter", label = "Clamp")]
#[children(
    min: f64 = 0.0 (
        label = "Min"
    );
    max: f64 = 1.0 (
        label = "Max"
    );
)]
pub struct ClampFilter {}

#[item("sm_filter", node = "sm_clamp_filter", from_struct)]
impl Node for ClampFilter {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
