use super::*;

#[test]
fn actual_sdk_names_and_handles_follow_independent_publication_lifetimes() {
    let mut first = Publication::default();
    let mut second = Publication::default();
    let native_first = SenderIdentity {
        name: "Renamed SDK sender_1".into(),
        shared_handle: 10,
    };
    let native_second = SenderIdentity {
        name: "Other SDK sender".into(),
        shared_handle: 20,
    };
    let external = SenderIdentity {
        name: native_first.name.clone(),
        shared_handle: 30,
    };
    let observed = [external.clone(), native_second.clone(), native_first.clone()];

    let before = revision();
    let first_send = first.before_first_send().unwrap();
    let second_send = second.before_first_send().unwrap();
    assert!(internal_names(revision(), &observed).is_none());
    first.complete_first_send(first_send, Some(native_first.clone()));
    assert!(!is_external(revision(), &external.name, external.shared_handle));
    second.complete_first_send(second_send, Some(native_second.clone()));
    assert!(first.before_first_send().is_none());
    assert!(second.before_first_send().is_none());
    assert!(internal_names(before, &observed).is_none());
    assert_eq!(
        internal_names(revision(), &observed).unwrap(),
        [native_second.name.clone(), native_first.name.clone()]
    );
    assert!(is_external(revision(), &external.name, external.shared_handle));
    assert!(!is_external(revision(), &native_first.name, native_first.shared_handle));
    assert!(is_external(
        revision(),
        "Renamed SDK sender",
        native_first.shared_handle
    ));

    let before_retirement = revision();
    first.retire(|| assert!(internal_names(revision(), &observed).is_none()));
    assert!(internal_names(before_retirement, &observed).is_none());
    assert_eq!(internal_names(revision(), &observed).unwrap(), [native_second.name]);
    drop(second);
    assert!(internal_names(revision(), &observed).unwrap().is_empty());
}
