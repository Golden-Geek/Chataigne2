use crate::events::Event;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck};
use crate::process_ctx::ProcessCtx;

use super::{
    DeclId, EventPropagation, Node, NodeData, PARAMETER_CONTROL_ITEM_KIND, PARAMETER_EXPRESSION_CONTROL_DECL_ID, PARAMETER_EXPRESSION_CONTROL_NODE_TYPE, PARAMETER_EXPRESSION_SOURCE_DECL_ID,
};

fn make_expression_source_parameter() -> Parameter {
    let mut source = Parameter::new("Expression", ParamValue::Str(String::new()), ParameterChangeCheck::ValueChange);
    source.node_data_mut().meta.decl_id = DeclId(PARAMETER_EXPRESSION_SOURCE_DECL_ID.to_string());
    source.node_data_mut().meta.can_be_disabled = false;
    source.control_modes_enabled = false;
    source
}

/// Internal control node attached to one parameter for expression behavior.
pub struct ParameterExpressionControlNode {
    node_data: NodeData,
}

impl ParameterExpressionControlNode {
    /// Creates a new expression-control node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_EXPRESSION_CONTROL_DECL_ID.to_string());
        Self { node_data }
    }
}

impl Node for ParameterExpressionControlNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_EXPRESSION_CONTROL_NODE_TYPE
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_CONTROL_ITEM_KIND
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        if !super::parameter_child_exists(ctx, self.id(), PARAMETER_EXPRESSION_SOURCE_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_expression_source_parameter()), None);
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}
