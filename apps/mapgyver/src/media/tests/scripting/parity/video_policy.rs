use super::*;
use crate::media::video::protocol::{VideoEdit, VideoOnActivate, VideoOnDeactivate};

#[test]
fn typed_video_policies_have_the_same_committed_result_for_every_caller() {
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let solid = create(&mut engine, MediaKind::SolidColor);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    for media in [video.media, solid.media] {
        for edit in [
            VideoEdit::OnActivate {
                policy: VideoOnActivate::PlayFromStart,
            },
            VideoEdit::OnActivate {
                policy: VideoOnActivate::Resume,
            },
            VideoEdit::OnActivate {
                policy: VideoOnActivate::Nothing,
            },
            VideoEdit::OnDeactivate {
                policy: VideoOnDeactivate::Stop,
            },
            VideoEdit::OnDeactivate {
                policy: VideoOnDeactivate::Pause,
            },
            VideoEdit::OnDeactivate {
                policy: VideoOnDeactivate::Nothing,
            },
        ] {
            let mut expected = None;
            for caller in CALLERS {
                let mut engine = loaded(&json);
                let result = run(
                    &mut engine,
                    caller,
                    MediaEdit::Video {
                        media,
                        edit: edit.clone(),
                    },
                );
                let actual = (result, authored(&engine));
                if let Some(expected) = &expected {
                    assert_eq!(&actual, expected, "{caller:?}, {edit:?}");
                } else {
                    expected = Some(actual);
                }
            }
        }
    }
}
