//! Typed app-wide options; Golden owns their Inspector and separate Preferences persistence.
use golden_core::{
    edit::NodeTree,
    node,
    node::{DeclId, Node},
    parameter::Enum,
    process_ctx::ProcessCtx,
};

mod catalog;
#[cfg(all(windows, feature = "native-video"))]
pub(crate) mod native;
#[cfg(any(test, all(windows, feature = "native-video")))]
pub(crate) use catalog::Source;

pub(crate) const VIDEO_DECL_ID: &str = "mapgyver_video";
pub(crate) const AUTOMATIC: &str = "Automatic";

#[node("mapgyver.preferences.video", label = "Video", collapsed = false)]
#[children(
    adapter: Enum = "Automatic" (
        label = "GPU",
        description = "Automatic prefers the high-performance GPU. Changes apply after restarting MapGyver. A missing explicit GPU never falls back to another adapter.",
        enum_options = crate::parameters::enum_options(&[crate::preferences::AUTOMATIC])
    );
)]
pub struct VideoPreferencesNode {
    #[state(default = catalog::Inbox::default())]
    inbox: catalog::Inbox,
}

#[node("mapgyver.preferences.video", from_struct)]
impl Node for VideoPreferencesNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: golden_core::node::NodeCreationContext) {
        if let Err(error) = self.inbox.register(ctx) {
            ctx.set_node_warning(self.id(), error);
        }
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        if let Some(update) = self.inbox.take() {
            update.apply(ctx);
        }
    }
    fn worker_wake_when_disabled(&self) -> bool {
        true
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.inbox = Default::default();
    }
}

pub(crate) fn tree(default_data_folder: String) -> NodeTree {
    let mut root = golden_core::app::default_preferences_tree(default_data_folder);
    let mut video = VideoPreferencesNode::new();
    video.node_data_mut().meta.decl_id = DeclId(VIDEO_DECL_ID.into());
    root.push_child(NodeTree::new(video));
    root
}

#[cfg(test)]
mod tests;
