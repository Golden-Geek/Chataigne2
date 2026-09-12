use crate::{kernel_profile::profile_kernel, processor_kernel_profile_snapshot};

#[test]
fn kernel_profile_counts_only_wrapped_work_on_the_current_thread() {
    let before = processor_kernel_profile_snapshot();
    let value = profile_kernel(|| 42);
    let after = processor_kernel_profile_snapshot();

    assert_eq!(value, 42);
    assert_eq!(after.evaluations - before.evaluations, 1);
    assert!(after.elapsed_ns >= before.elapsed_ns);
}
