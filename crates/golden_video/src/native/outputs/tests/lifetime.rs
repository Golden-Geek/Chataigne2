use std::cell::Cell;
use std::rc::Rc;

use super::super::lifetime::Endpoint;

struct NativeObject(Rc<Cell<usize>>);
impl Drop for NativeObject {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

#[test]
fn failed_endpoint_retains_queued_use_and_never_overlaps_its_replacement() {
    let live = Rc::new(Cell::new(0));
    let create = || {
        assert_eq!(live.get(), 0, "a replacement cannot overlap the charged slot");
        live.set(live.get() + 1);
        Ok::<_, ()>(NativeObject(live.clone()))
    };
    let mut slot = Endpoint::new();
    assert!(slot.create(create).unwrap());
    assert!(slot.begin_use(10).is_some());
    slot.fail();
    assert!(
        slot.begin_use(11).is_none(),
        "failed ownership is withdrawn immediately"
    );
    assert!(!slot.retire(9));
    assert_eq!(live.get(), 1, "a failed native call can already have queued GPU work");
    assert!(
        !slot
            .create(|| -> Result<_, ()> { panic!("factory must await completion") })
            .unwrap()
    );
    assert!(slot.retire(10));
    assert_eq!(live.get(), 0);
    assert!(slot.create(create).unwrap());
    assert_eq!(live.get(), 1);
    slot.clear();
    assert_eq!(live.get(), 0);
}
