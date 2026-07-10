use super::*;

#[test]
fn revisions_advance_without_wrapping() {
    assert_eq!(Revision::new(41).next(), Some(Revision::new(42)));
    assert_eq!(Revision::new(u64::MAX).next(), None);
}

#[test]
fn change_sets_own_one_precise_revision_transition() {
    let changes = ChangeSet::new(Revision::new(7), vec!["updated"]).expect("revision should advance");
    assert_eq!(changes.before, Revision::new(7));
    assert_eq!(changes.after, Revision::new(8));
    assert_eq!(changes.changes, ["updated"]);
}
