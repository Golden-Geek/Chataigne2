use super::super::{
    tests::{create_mqtt_module, find_path},
    MqttModule, MqttSubscription,
};
use super::*;
use golden_core::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use golden_io::{RetirementPool, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

mod collection;
mod lifetimes;
mod source;

fn module(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut MqttModule {
    let crate::app::AppNode::MqttModule(module) = engine.nodes.get_mut(id).unwrap() else {
        unreachable!()
    };
    module
}

fn set(engine: &mut crate::app::AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}

fn draft(snapshot: &Arc<ProcessTreeSnapshot>, id: NodeId, slots: &RetirementPool) -> (Preparation, preparation::Job) {
    Preparation::draft(snapshot, id, None, WorkerNotifier::new(|| {}), slots).unwrap()
}

fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}

fn build(snapshot: &ProcessTreeSnapshot, id: NodeId) -> Result<MqttTransportConfig, Error> {
    collect::build(snapshot, id, &AtomicBool::new(false))
}
