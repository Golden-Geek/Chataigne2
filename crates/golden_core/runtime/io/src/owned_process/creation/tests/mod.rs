use super::*;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Owner(Arc<AtomicUsize>);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn mapping_and_extracting_a_created_owner_preserves_it_and_the_original_os_error() {
    let drops = Arc::new(AtomicUsize::new(0));
    let error = CreateError::with_owner(io::Error::from_raw_os_error(5), Owner(drops.clone()));
    let error = error.map_owner(Some);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(error.error().raw_os_error(), Some(5));
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .raw_os_error(),
        Some(5)
    );
    let (cause, owner) = error.into_parts();
    assert_eq!(cause.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(owner);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn dropping_a_creation_error_releases_its_retained_owner_exactly_once() {
    let drops = Arc::new(AtomicUsize::new(0));
    let error = CreateError::with_owner(io::Error::other("verification rejected"), Owner(drops.clone()));
    drop(error);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn failure_before_creation_returns_the_os_error_without_an_owner() {
    let error: CreateError<Owner> = io::Error::from_raw_os_error(2).into();
    assert!(error.to_string().contains("os error 2"));
    let (cause, owner) = error.into_parts();
    assert_eq!(cause.raw_os_error(), Some(2));
    assert!(owner.is_none());
}
