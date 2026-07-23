use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{RealtimeScope, acknowledged_plan_exchange, is_realtime_thread};

#[test]
fn realtime_scope_is_nested_and_reclamation_guard_fails_closed() {
    assert!(!is_realtime_thread());
    let (mut publisher, _realtime) = acknowledged_plan_exchange(Box::new(0_u8));
    publisher.publish(Box::new(1)).unwrap();

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _outer = RealtimeScope::enter();
        assert!(is_realtime_thread());
        {
            let _inner = RealtimeScope::enter();
            assert!(is_realtime_thread());
        }
        assert!(is_realtime_thread());
        publisher.reclaim_acknowledged();
    }));

    assert!(result.is_err());
    assert!(!is_realtime_thread());
}
