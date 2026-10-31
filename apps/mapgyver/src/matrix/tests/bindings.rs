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
        id: uuid::Uuid::new_v4(),
        column: ColumnId(uuid()),
        version: 10,
    };
    let row = LayerId(uuid());
    let owner = reference();
    let source = CellSource {
        layer: row,
        source: reference(),
        version: 10,
    };
    assert_eq!(
        bindings.resolve(clip, row, owner, Some(&source)).ok().flatten(),
        Some(source.source)
    );
    let edited = CellSource {
        layer: row,
        source: reference(),
        version: 11,
    };
    assert_eq!(
        bindings.resolve(clip, row, owner, Some(&edited)).ok().flatten(),
        Some(source.source)
    );
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
    bindings.retain(std::iter::once(next.id));
    assert_eq!(bindings.edges(), vec![(owner, edited.source)]);
    for _ in 1..MAX_BINDINGS {
        assert!(bindings.resolve(next, LayerId(uuid()), owner, Some(&edited)).is_ok());
    }
    assert_eq!(bindings.edges().len(), MAX_BINDINGS);
    assert!(bindings.resolve(next, LayerId(uuid()), owner, Some(&edited)).is_err());
    bindings.retain(std::iter::empty());
    assert!(bindings.edges().is_empty());
    assert!(bindings.resolve(next, row, owner, Some(&edited)).is_ok());
}
