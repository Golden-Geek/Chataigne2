use super::{Role, sync_interval};
use mapgyver_native_probe::workload::PresentationPolicy;

#[test]
fn comparison_changes_only_native_output_and_leaves_editor_on_latest() {
    for policy in [PresentationPolicy::Latest, PresentationPolicy::OutputVblank] {
        assert_eq!(sync_interval(Role::Editor, policy), 0, "editor policy {policy:?}");
    }
    assert_eq!(sync_interval(Role::NativeOutput, PresentationPolicy::Latest), 0);
    assert_eq!(sync_interval(Role::NativeOutput, PresentationPolicy::OutputVblank), 1);
}
