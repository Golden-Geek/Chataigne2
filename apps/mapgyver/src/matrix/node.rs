use super::protocol::*;
use golden_core::{
    node::{Node, NodeData, UserContainerRules},
    process_ctx::ProcessCtx,
};
use golden_io::RevisionSource;
use serde_json::Value;
use std::any::Any;
use std::sync::OnceLock;
pub const LAYER_TYPE: &str = "mapgyver.matrix.layer";
pub const GROUP_TYPE: &str = "mapgyver.matrix.group";
pub const COLUMN_TYPE: &str = "mapgyver.matrix.column";
pub const CELL_TYPE: &str = "mapgyver.matrix.cell";
pub struct ElementNode {
    data: NodeData,
    definition: ElementDefinition,
    revision: RevisionSource,
    runtime: OnceLock<super::playback::RuntimeBinding>,
}
impl ElementNode {
    pub(crate) fn new(label: String, definition: ElementDefinition) -> Self {
        Self {
            data: NodeData::new(label),
            definition,
            revision: RevisionSource::default(),
            runtime: OnceLock::new(),
        }
    }
    pub(crate) fn definition(&self) -> &ElementDefinition {
        &self.definition
    }
    pub(super) fn bind_runtime(&self, binding: super::playback::RuntimeBinding) {
        let _ = self.runtime.set(binding);
    }
    fn invalidate(&self) {
        self.revision.advance();
        if let Some(binding) = self.runtime.get() {
            binding.invalidate();
        }
    }
    pub(crate) fn set_definition(&mut self, definition: ElementDefinition) -> Result<(), String> {
        if std::mem::discriminant(&definition) != std::mem::discriminant(&self.definition) {
            return Err("Matrix element kind cannot change".into());
        }
        if let ElementDefinition::Layer { settings } | ElementDefinition::Group { settings } = &definition {
            settings.validate()?;
        }
        if self.definition != definition {
            self.invalidate();
            self.definition = definition;
        }
        Ok(())
    }
}
fn canonical(node_type: &str) -> Option<ElementDefinition> {
    Some(match node_type {
        LAYER_TYPE => ElementDefinition::Layer {
            settings: CompositeSettings::default(),
        },
        GROUP_TYPE => ElementDefinition::Group {
            settings: CompositeSettings::default(),
        },
        COLUMN_TYPE => ElementDefinition::Column {
            definition: ColumnDefinition::default(),
        },
        CELL_TYPE => ElementDefinition::Cell {
            definition: CellDefinition {
                column: ColumnId(golden_core::model::NodeUuid::nil()),
                source: None,
            },
        },
        _ => return None,
    })
}
pub(crate) fn create(node_type: &str) -> Option<ElementNode> {
    canonical(node_type).map(|definition| ElementNode::new("Matrix element".into(), definition))
}
impl Node for ElementNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn get_type(&self) -> &str {
        match self.definition {
            ElementDefinition::Layer { .. } => LAYER_TYPE,
            ElementDefinition::Group { .. } => GROUP_TYPE,
            ElementDefinition::Column { .. } => COLUMN_TYPE,
            ElementDefinition::Cell { .. } => CELL_TYPE,
        }
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        match self.definition {
            ElementDefinition::Group { .. } => Some(UserContainerRules::new(&[GROUP_TYPE, LAYER_TYPE])),
            ElementDefinition::Layer { .. } => Some(UserContainerRules::new(&[CELL_TYPE])),
            _ => None,
        }
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create(node_type).map(|node| Box::new(node) as Box<dyn Node>)
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.invalidate();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn project_encode_data(&self) -> Result<Value, String> {
        match &self.definition {
            ElementDefinition::Layer { settings } | ElementDefinition::Group { settings } => {
                serde_json::to_value(settings)
            }
            ElementDefinition::Column { definition } => serde_json::to_value(definition),
            ElementDefinition::Cell { definition } => serde_json::to_value(definition),
        }
        .map_err(|error| error.to_string())
    }
    fn project_decode_data(&mut self, data: &Value) -> Result<(), String> {
        if data.is_null() {
            return self.set_definition(canonical(self.get_type()).unwrap());
        }
        // The registered node type owns kind; sparse payloads contain only that kind's data.
        let definition = match &self.definition {
            ElementDefinition::Layer { .. } => ElementDefinition::Layer {
                settings: serde_json::from_value(data.clone()).map_err(|error| error.to_string())?,
            },
            ElementDefinition::Group { .. } => ElementDefinition::Group {
                settings: serde_json::from_value(data.clone()).map_err(|error| error.to_string())?,
            },
            ElementDefinition::Column { .. } => ElementDefinition::Column {
                definition: serde_json::from_value(data.clone()).map_err(|error| error.to_string())?,
            },
            ElementDefinition::Cell { .. } => ElementDefinition::Cell {
                definition: serde_json::from_value(data.clone()).map_err(|error| error.to_string())?,
            },
        };
        self.set_definition(definition)
    }
    fn project_create(node_type: &str) -> Option<Self> {
        create(node_type)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        if let ElementDefinition::Layer { settings } | ElementDefinition::Group { settings } = &self.definition {
            settings.validate()?;
        }
        Ok(())
    }
}
