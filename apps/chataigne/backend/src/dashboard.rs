use golden_model::EntityId;
use golden_values::StableValueRef;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dashboard {
    pub id: EntityId,
    pub name: SmolStr,
    pub controls: Vec<DashboardControl>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DashboardControl {
    pub id: EntityId,
    pub label: SmolStr,
    pub route: DashboardRoute,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DashboardRoute {
    Value(StableValueRef),
    Command { module: EntityId, command: SmolStr },
}
