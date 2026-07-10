use super::*;

#[test]
fn new_projects_begin_at_the_canonical_zero_revision() {
    assert_eq!(ChataigneProjectIdentity::new().revision, Revision::ZERO);
}
