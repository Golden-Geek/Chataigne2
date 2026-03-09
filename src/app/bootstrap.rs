use golden_core::node::Folder;

use super::{AppEngine, AppNode};

pub(super) fn build_engine() -> AppEngine {
    let root: AppNode = Folder::new("Root").into();
    let mut engine = AppEngine::new(root);
    super::nodes_module_demo::register_demo_reference_filters(&mut engine);
    engine
}
