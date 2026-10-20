use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use golden_parameters::{
    NodeReference, ParamValue, coerce_param_value_for_target_owned, coerce_param_value_for_target_ref,
    compatibility_for_binding_values,
};

struct TrackedAllocator;
thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

fn record() {
    let _ = ALLOCATIONS.try_with(|counter| {
        if let Some(count) = counter.get() {
            counter.set(Some(count + 1));
        }
    });
}

unsafe impl GlobalAlloc for TrackedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: TrackedAllocator = TrackedAllocator;

fn measure<T>(callback: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.with(|counter| assert!(counter.replace(Some(0)).is_none()));
    let result = callback();
    let count = ALLOCATIONS.with(|counter| counter.replace(None).unwrap());
    (result, count)
}

#[test]
fn large_unchanged_and_rejected_inputs_are_not_allocated_by_coercion() {
    let value = ParamValue::Enum("device".repeat(1024 * 1024));
    let (result, count) = measure(|| coerce_param_value_for_target_ref(&value, &value, None));
    assert!(result.is_some());
    assert_eq!(count, 0);
    let (result, count) = measure(|| coerce_param_value_for_target_ref(&value, &ParamValue::Int(0), None));
    assert!(result.is_none());
    assert_eq!(count, 0);
    let (result, count) = measure(|| coerce_param_value_for_target_owned(value, &ParamValue::Str(String::new()), None));
    assert!(matches!(result, Some(ParamValue::Str(_))));
    assert_eq!(count, 0);
}

#[test]
fn reference_storage_moves_and_direct_compatibility_does_not_clone_hints() {
    let reference = NodeReference {
        cached_name: Some("display".repeat(1024 * 1024)),
        relative_path_from_root: vec!["recovery".repeat(1024 * 1024)],
        ..Default::default()
    };
    let value = ParamValue::Reference(reference);
    let target = ParamValue::Reference(NodeReference::default());
    let (result, count) = measure(|| compatibility_for_binding_values(&value, &target));
    assert!(result.direct);
    assert_eq!(count, 0);
    let (result, count) = measure(|| coerce_param_value_for_target_owned(value, &target, None));
    assert!(result.is_some());
    assert_eq!(count, 0);
}
