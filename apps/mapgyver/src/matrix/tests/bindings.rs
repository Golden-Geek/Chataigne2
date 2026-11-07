use super::super::{
    bindings::{Bindings, MAX_BINDINGS},
    projection::CellSource,
    timeline::Clip,
};
use super::*;

fn uuid() -> golden_core::model::NodeUuid {
    golden_core::model::NodeUuid(uuid::Uuid::new_v4())
}
fn reference() -> ChannelReference {
    ChannelReference {
        media: MediaId(uuid()),
        channel: ChannelId(uuid()),
    }
}

#[test]
fn clip_bindings_pin_sources_and_empty_decisions_with_a_fixed_recoverable_metadata_bound() {
    let mut bindings = Bindings::default();
    let clip = Clip {
        order: 0,
        individual: false,
        id: uuid::Uuid::new_v4(),
        column: ColumnId(uuid()),
        version: 10,
    };
    let row = LayerId(uuid());
    let owner = reference();
    let source = CellSource {
        settings: CellSettings {
            position: [2.0, 3.0],
            opacity: 0.25,
            ..Default::default()
        },
        layer: row,
        source: {
            let reference = reference();
            crate::media::MediaSource {
                media: reference.media,
                channel: Some(reference.channel),
            }
        },
        version: 10,
    };
    assert_eq!(
        bindings.resolve(clip, row, owner, Some(&source)).ok().flatten(),
        Some(source.source)
    );
    let edited = CellSource {
        settings: CellSettings::default(),
        layer: row,
        source: {
            let reference = reference();
            crate::media::MediaSource {
                media: reference.media,
                channel: Some(reference.channel),
            }
        },
        version: 11,
    };
    assert_eq!(
        bindings.resolve(clip, row, owner, Some(&edited)).ok().flatten(),
        Some(source.source)
    );
    assert_eq!(bindings.settings(clip, row), source.settings);
    let live_edit = CellSource {
        settings: CellSettings {
            position: [4.0, -2.0],
            opacity: 0.5,
            ..Default::default()
        },
        layer: row,
        source: source.source,
        version: source.version,
    };
    bindings.resolve(clip, row, owner, Some(&live_edit)).ok().unwrap();
    assert_eq!(bindings.settings(clip, row), live_edit.settings);
    bindings.resolve(clip, row, owner, None).ok().unwrap();
    assert_eq!(bindings.settings(clip, row), live_edit.settings);
    let new_row = LayerId(uuid());
    assert!(
        bindings
            .resolve(clip, new_row, owner, Some(&edited))
            .ok()
            .flatten()
            .is_none()
    );
    assert!(
        bindings
            .resolve(clip, new_row, owner, Some(&source))
            .ok()
            .flatten()
            .is_none()
    );
    let next = Clip {
        id: uuid::Uuid::new_v4(),
        version: 11,
        ..clip
    };
    assert_eq!(
        bindings.resolve(next, row, owner, Some(&edited)).ok().flatten(),
        Some(edited.source)
    );
    let mut frame = super::super::timeline::Frame::default();
    frame.clips.push(super::super::timeline::Contribution {
        clip: next,
        from: 1.0,
        to: 1.0,
    });
    bindings.retain_frame(&frame);
    assert_eq!(
        bindings.edges(),
        vec![(
            owner,
            ChannelReference {
                media: edited.source.media,
                channel: edited.source.channel.unwrap()
            }
        )]
    );
    for _ in 1..MAX_BINDINGS {
        assert!(bindings.resolve(next, LayerId(uuid()), owner, Some(&edited)).is_ok());
    }
    assert_eq!(bindings.edges().len(), MAX_BINDINGS);
    assert!(bindings.resolve(next, LayerId(uuid()), owner, Some(&edited)).is_err());
    bindings.retain_frame(&super::super::timeline::Frame::default());
    assert!(bindings.edges().is_empty());
    assert!(bindings.resolve(next, row, owner, Some(&edited)).is_ok());
}
