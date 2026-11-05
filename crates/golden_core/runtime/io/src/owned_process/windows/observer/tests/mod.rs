use super::*;

#[test]
fn only_the_initial_primary_thread_first_chance_breakpoint_is_consumed() {
    let mut breakpoint = InitialBreakpoint::new(7, false);
    assert!(!breakpoint.consume(8, true, EXCEPTION_BREAKPOINT));
    assert!(!breakpoint.consume(7, false, EXCEPTION_BREAKPOINT));
    assert!(!breakpoint.consume(7, true, NTSTATUS(0xe0420001_u32 as i32)));
    assert!(!breakpoint.consume(7, true, STATUS_WX86_BREAKPOINT));
    assert!(breakpoint.consume(7, true, EXCEPTION_BREAKPOINT));
    assert!(!breakpoint.consume(7, true, EXCEPTION_BREAKPOINT));
}

#[test]
fn wow64_loader_breakpoints_are_distinct_and_each_is_consumed_once() {
    let mut breakpoint = InitialBreakpoint::new(7, true);
    assert!(breakpoint.consume(7, true, EXCEPTION_BREAKPOINT));
    assert!(breakpoint.consume(7, true, STATUS_WX86_BREAKPOINT));
    assert!(!breakpoint.consume(7, true, STATUS_WX86_BREAKPOINT));
    assert!(!breakpoint.consume(7, true, EXCEPTION_BREAKPOINT));
}
