use super::*;
use crate::media::protocol::{ChannelId, ChannelReference, MediaId};
use golden_core::model::NodeUuid;
use golden_io::RevisionSource;
use std::sync::Arc;

fn reference() -> ChannelReference {
    ChannelReference {
        media: MediaId(NodeUuid(uuid::Uuid::new_v4())),
        channel: ChannelId(NodeUuid(uuid::Uuid::new_v4())),
    }
}
#[test]
fn runtime_metadata_is_bounded_replaced_and_fenced_by_revision_and_owner() {
    let view = RuntimeResolutions::default();
    let reference = reference();
    let revision = RevisionSource::default();
    let owner = Arc::new(());
    let snapshot = || {
        ResolutionSnapshot::new(
            std::iter::once((
                reference,
                ChannelResolution {
                    extent: Some(Extent2D::new(10, 20).unwrap()),
                    availability: ChannelAvailability::Ready,
                },
            )),
            Arc::from([revision.reader().guard_at(revision.reader().current())]),
            Arc::downgrade(&owner),
        )
        .unwrap()
    };
    view.publish(snapshot());
    assert_eq!(view.extent(reference), Some(Extent2D::new(10, 20).unwrap()));
    revision.advance();
    assert_eq!(view.extent(reference), None);
    view.publish(snapshot());
    assert!(view.extent(reference).is_some());
    drop(owner);
    assert_eq!(view.extent(reference), None);
    let owner = Arc::new(());
    let resolution = ChannelResolution {
        extent: None,
        availability: ChannelAvailability::Unavailable,
    };
    assert!(
        ResolutionSnapshot::new(
            (0..4097).map(|_| (self::reference(), resolution)),
            Arc::from([]),
            Arc::downgrade(&owner)
        )
        .is_err()
    );
    view.publish(ResolutionSnapshot::new(std::iter::empty(), Arc::from([]), Arc::downgrade(&owner)).unwrap());
    assert_eq!(view.extent(reference), None);
}
