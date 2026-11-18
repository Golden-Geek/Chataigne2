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
    let (senders, truncated) = collect(
        NonZeroUsize::new(64).unwrap(),
        || i32::MAX,
        |index, buffer| {
            calls.push(index);
            write(buffer, format!("Sender {index}").as_bytes())
        },
        |_| Some(1),
    )
    .unwrap();
    assert_eq!(calls, (0..64).collect::<Vec<_>>());
    assert_eq!(senders.len(), 64);
    assert_eq!(senders[2].name, "Sender 2");
    assert!(truncated);
    assert_eq!(
        collect(
            NonZeroUsize::new(257).unwrap(),
            || panic!("must not enter SDK"),
            |_, _| panic!(),
            |_| panic!()
        ),
        Err(DiscoveryError::Limit)
    );
}

#[test]
fn names_stay_exact_and_churn_invalid_utf8_and_missing_terminators_are_explicit() {
    let values: [&[u8]; 7] = [
        b"Zebra ",
        b" Alpha",
        b" Alpha",
        &[0xff],
        b"bad\0name",
        b"",
        b"Missing metadata",
    ];
    let mut info_calls = Vec::new();
    let (senders, truncated) = collect(
        NonZeroUsize::new(16).unwrap(),
        || 8,
        |index, buffer| {
            if index == 7 {
                buffer.fill(1);
                return 256;
            }
            write(buffer, values[index as usize])
        },
        |name| {
            info_calls.push(name.to_str().unwrap().to_owned());
            (name != c"Missing metadata").then_some(info_calls.len() as u32)
        },
    )
    .unwrap();
    let expected = [
        SenderIdentity {
            name: "Zebra ".into(),
            shared_handle: 1,
        },
        SenderIdentity {
            name: " Alpha".into(),
            shared_handle: 2,
        },
    ];
    assert_eq!(senders, expected);
    assert_eq!(info_calls, ["Zebra ", " Alpha", "Missing metadata"]);
    assert!(truncated);
    let (empty, truncated) = collect(NonZeroUsize::MIN, || 0, |_, _| panic!(), |_| panic!()).unwrap();
    assert!(empty.is_empty());
    assert!(!truncated);
    assert_eq!(
        collect(NonZeroUsize::MIN, || -1, |_, _| panic!(), |_| panic!()),
        Err(DiscoveryError::Unavailable)
    );
}
