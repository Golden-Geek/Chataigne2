use super::*;

fn write(buffer: &mut [c_char; 256], value: &[u8]) -> i32 {
    for (target, byte) in buffer.iter_mut().zip(value) {
        *target = *byte as c_char;
    }
    value.len() as i32
}

#[test]
fn advertised_registry_size_cannot_expand_the_native_call_or_result_bound() {
    let mut calls = Vec::new();
    let catalog = collect(
        NonZeroUsize::new(64).unwrap(),
        || i32::MAX,
        |index, buffer| {
            calls.push(index);
            write(buffer, format!("Sender {index}").as_bytes())
        },
    )
    .unwrap();
    assert_eq!(calls, (0..64).collect::<Vec<_>>());
    assert_eq!(catalog.names.len(), 64);
    assert!(catalog.truncated);
    assert_eq!(
        collect(
            NonZeroUsize::new(257).unwrap(),
            || panic!("must not enter SDK"),
            |_, _| panic!()
        ),
        Err(DiscoveryError::Limit)
    );
}

#[test]
fn names_stay_exact_and_churn_invalid_utf8_and_missing_terminators_are_explicit() {
    let values: [&[u8]; 6] = [b" Zebra ", b"Alpha", b"Alpha", &[0xff], b"bad\0name", b""];
    let catalog = collect(
        NonZeroUsize::new(16).unwrap(),
        || 7,
        |index, buffer| {
            if index == 6 {
                buffer.fill(1);
                return 256;
            }
            write(buffer, values[index as usize])
        },
    )
    .unwrap();
    assert_eq!(catalog.names, [" Zebra ", "Alpha"]);
    assert!(catalog.truncated);
    let empty = collect(NonZeroUsize::MIN, || 0, |_, _| panic!()).unwrap();
    assert!(empty.names.is_empty());
    assert!(!empty.truncated);
    assert_eq!(
        collect(NonZeroUsize::MIN, || -1, |_, _| panic!()),
        Err(DiscoveryError::Unavailable)
    );
}
