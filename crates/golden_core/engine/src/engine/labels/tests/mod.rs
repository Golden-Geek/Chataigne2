use super::*;

#[test]
fn reservations_preserve_first_available_suffixes_and_numbered_preferences() {
    let mut labels = UniqueLabels::new(["Gain", "Gain 2", "Gain 4"].map(str::to_owned));
    assert_eq!(labels.reserve(" Gain "), "Gain 3");
    assert_eq!(labels.reserve("Gain 4"), "Gain 5");
    assert_eq!(labels.reserve("Gain"), "Gain 6");
    assert_eq!(labels.reserve(""), "Item");
    assert_eq!(labels.reserve(""), "Item 2");
    let maximum = format!("Gain {}", u64::MAX);
    assert_eq!(labels.reserve(&maximum), maximum);
    assert_eq!(labels.reserve(&maximum), format!("{maximum} Copy"));
}

#[test]
fn numbered_batch_copies_share_successor_search_instead_of_rescanning_the_prefix() {
    let mut labels = UniqueLabels::new((2..=10_000).map(|index| format!("Gain {index}")).chain(["Gain".into()]));
    assert_eq!(labels.reserve("Gain"), "Gain 10001");
    for index in 2..=10_000 {
        assert_eq!(
            labels.reserve(&format!("Gain {index}")),
            format!("Gain {}", 10_000 + index)
        );
    }
    assert_eq!(labels.used.len(), 20_000);
    assert!(labels.successors["Gain"].len() <= 20_000);
}
