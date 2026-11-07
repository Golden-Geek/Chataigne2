use super::*;
use crate::screens::protocol::PhysicalRectangle;
use golden_core::model::NodeUuid;
use golden_video::Extent2D;

fn screen(index: u128) -> ScreenId {
    ScreenId(NodeUuid(uuid::Uuid::from_u128(index)))
}
fn display(id: &str, edid: Option<&str>, name: &str, index: u32, left: i32) -> DisplayInfo {
    DisplayInfo {
        refresh_rate: None,
        id: id.into(),
        edid: edid.map(str::to_owned),
        name: name.into(),
        index,
        primary: false,
        bounds: PhysicalRectangle {
            left,
            top: 0,
            extent: Extent2D::new(1920, 1080).unwrap(),
        },
    }
}
fn resolved<'a>(
    matches: &HashMap<ScreenId, DisplayMatch>,
    screen: ScreenId,
    catalog: &'a [DisplayInfo],
) -> &'a DisplayInfo {
    &catalog[matches[&screen].display_index]
}

#[test]
fn edid_reclaims_original_panels_after_port_paths_os_numbers_and_layout_change() {
    let old = [
        display("old-a", Some("edid-a"), "Twin", 1, 0),
        display("old-b", Some("edid-b"), "Twin", 2, 1920),
    ];
    let requests = [
        (screen(1), DisplayIdentityHint::from(&old[0])),
        (screen(2), DisplayIdentityHint::from(&old[1])),
    ];
    let catalog = [
        display("new-b", Some("edid-b"), "Twin", 1, 0),
        display("new-a", Some("edid-a"), "Twin", 2, 1920),
    ];
    let original = requests.clone();
    let matches = match_displays(&requests, &catalog);
    assert_eq!(resolved(&matches, screen(1), &catalog).id, "new-a");
    assert_eq!(resolved(&matches, screen(2), &catalog).id, "new-b");
    assert!(
        matches
            .values()
            .all(|value| value.reason == DisplayMatchReason::Edid && !value.ambiguous)
    );
    assert_eq!(requests, original, "runtime fallback never rewrites authored evidence");
}

#[test]
fn partial_reconnect_reserves_peer_edid_before_a_missing_screen_uses_any_weaker_fallback() {
    let first = display("old-a", Some("edid-a"), "Same", 1, 0);
    let peer = display("old-b", Some("edid-b"), "Same", 2, 1920);
    let requests = [
        (screen(1), DisplayIdentityHint::from(&first)),
        (screen(2), DisplayIdentityHint::from(&peer)),
    ];
    let catalog = [display("new-b", Some("edid-b"), "Same", 1, 0)];
    let matches = match_displays(&requests, &catalog);
    assert_eq!(matches.len(), 1);
    assert!(!matches.contains_key(&screen(1)));
    assert_eq!(resolved(&matches, screen(2), &catalog).id, "new-b");
    let returning = [catalog[0].clone(), display("new-a", Some("edid-a"), "Same", 7, -1920)];
    let matches = match_displays(&requests, &returning);
    assert_eq!(resolved(&matches, screen(1), &returning).id, "new-a");
    assert_eq!(resolved(&matches, screen(2), &returning).id, "new-b");
}

#[test]
fn differing_edid_allows_tolerant_recovery_and_original_evidence_wins_when_it_returns() {
    let intended = display("old", Some("original"), "Studio Monitor", 2, 1920);
    let requests = [(screen(1), DisplayIdentityHint::from(&intended))];
    let substitute = display("replacement", Some("different"), "Studio Monitor", 2, 1920);
    let matches = match_displays(&requests, std::slice::from_ref(&substitute));
    assert_eq!(matches[&screen(1)].reason, DisplayMatchReason::Name);
    let catalog = [substitute, display("another-port", Some("original"), "Renamed", 1, 0)];
    let matches = match_displays(&requests, &catalog);
    assert_eq!(resolved(&matches, screen(1), &catalog).id, "another-port");
    assert_eq!(matches[&screen(1)].reason, DisplayMatchReason::Edid);
}

#[test]
fn cohort_allocation_can_move_an_equal_edid_choice_to_preserve_another_screens_exact_path() {
    let first = display("new-one", Some("shared"), "Twin", 1, 0);
    let second = display("new-two", Some("shared"), "Twin", 2, 1920);
    let requests = [
        (
            screen(1),
            DisplayIdentityHint {
                edid: Some("shared".into()),
                ..Default::default()
            },
        ),
        (
            screen(2),
            DisplayIdentityHint {
                identity: "new-one".into(),
                ..Default::default()
            },
        ),
    ];
    let catalog = [first, second];
    let matches = match_displays(&requests, &catalog);
    assert_eq!(resolved(&matches, screen(1), &catalog).id, "new-two");
    assert_eq!(resolved(&matches, screen(2), &catalog).id, "new-one");
    assert!(matches.values().all(|value| !value.ambiguous));
}

#[test]
fn duplicate_edid_and_name_use_saved_layout_before_reordered_os_indices() {
    let left = display("old-left", Some("no-serial"), "Twin", 1, -1920);
    let right = display("old-right", Some("no-serial"), "Twin", 2, 0);
    let requests = [
        (screen(1), DisplayIdentityHint::from(&left)),
        (screen(2), DisplayIdentityHint::from(&right)),
    ];
    let catalog = [
        display("new-right", Some("no-serial"), "Twin", 1, 0),
        display("new-left", Some("no-serial"), "Twin", 2, -1920),
    ];
    let matches = match_displays(&requests, &catalog);
    assert_eq!(resolved(&matches, screen(1), &catalog).id, "new-left");
    assert_eq!(resolved(&matches, screen(2), &catalog).id, "new-right");
    assert!(matches.values().all(|value| !value.ambiguous));
}

#[test]
fn indistinguishable_candidates_are_deterministic_and_flagged_without_claiming_physical_certainty() {
    let hint = DisplayIdentityHint {
        edid: Some("same".into()),
        name: "Twin".into(),
        ..Default::default()
    };
    let requests = [(screen(1), hint.clone()), (screen(2), hint)];
    let catalog = [
        display("a", Some("same"), "Twin", 0, 0),
        display("b", Some("same"), "Twin", 0, 0),
    ];
    let first = match_displays(&requests, &catalog);
    assert_eq!(first.len(), 2);
    assert!(first.values().all(|value| value.ambiguous));
    assert_ne!(first[&screen(1)].display_index, first[&screen(2)].display_index);
    let mut reversed = catalog.clone();
    reversed.reverse();
    let reordered = [
        (requests[1].0, requests[1].1.clone()),
        (requests[0].0, requests[0].1.clone()),
    ];
    let second = match_displays(&reordered, &reversed);
    for (screen, _) in requests {
        assert_eq!(
            resolved(&first, screen, &catalog).id,
            resolved(&second, screen, &reversed).id
        );
    }
}

#[test]
fn missing_metadata_uses_case_insensitive_path_then_name_layout_and_nearest_fallback() {
    let catalog = [
        display("PATH-A", None, "  Studio   Monitor ", 3, 0),
        display("b", None, "Other", 9, 1920),
    ];
    let path = [(
        screen(1),
        DisplayIdentityHint {
            identity: "path-a".into(),
            ..Default::default()
        },
    )];
    assert_eq!(
        match_displays(&path, &catalog)[&screen(1)].reason,
        DisplayMatchReason::Identity
    );
    let name = [(
        screen(1),
        DisplayIdentityHint {
            name: "studio monitor".into(),
            ..Default::default()
        },
    )];
    assert_eq!(
        match_displays(&name, &catalog)[&screen(1)].reason,
        DisplayMatchReason::Name
    );
    let mut hint = DisplayIdentityHint::from(&catalog[1]);
    hint.identity.clear();
    hint.name.clear();
    hint.index = 0;
    hint.bounds.as_mut().unwrap().left += 10;
    hint.bounds.as_mut().unwrap().extent = Extent2D::new(1000, 800).unwrap();
    let fallback = [(screen(1), hint)];
    let matches = match_displays(&fallback, &catalog);
    assert_eq!(resolved(&matches, screen(1), &catalog).id, "b");
    assert_eq!(matches[&screen(1)].reason, DisplayMatchReason::Fallback);
}

#[test]
fn empty_and_oversubscribed_cohorts_do_not_duplicate_a_physical_target() {
    let catalog = [display("a", None, "A", 1, 0)];
    assert!(match_displays(&[], &catalog).is_empty());
    assert!(match_displays(&[(screen(1), DisplayIdentityHint::default())], &[]).is_empty());
    let requests = (1..=128)
        .map(|id| (screen(id), DisplayIdentityHint::default()))
        .collect::<Vec<_>>();
    let matches = match_displays(&requests, &catalog);
    assert_eq!(matches.len(), 1);
    assert!(matches.values().all(|value| value.ambiguous));
}

#[test]
fn maximum_native_catalog_recovers_reversed_requests_without_greedy_or_spread_limits() {
    let catalog = (0..32)
        .map(|index| {
            display(
                &format!("p{index}"),
                Some(&format!("e{index}")),
                "Twin",
                index + 1,
                index as i32 * 1920,
            )
        })
        .collect::<Vec<_>>();
    let requests = catalog
        .iter()
        .enumerate()
        .rev()
        .map(|(index, display)| (screen(index as u128 + 1), DisplayIdentityHint::from(display)))
        .collect::<Vec<_>>();
    let matches = match_displays(&requests, &catalog);
    assert_eq!(matches.len(), 32);
    for (index, display) in catalog.iter().enumerate() {
        let selected = matches[&screen(index as u128 + 1)];
        assert_eq!(selected.display_index, index);
        assert!(!selected.ambiguous);
        assert_eq!(selected.reason, DisplayMatchReason::Edid);
        assert_eq!(resolved(&matches, screen(index as u128 + 1), &catalog), display);
    }
}

#[test]
fn bounded_assignment_and_ambiguity_scores_agree_with_exhaustive_small_cohorts() {
    fn exhaustive(scores: &[Vec<i128>], excluded: Option<(usize, usize)>, row: usize, used: u64) -> i128 {
        if row == scores.len() {
            return 0;
        }
        (0..scores[0].len())
            .filter(|&column| used & (1 << column) == 0 && excluded != Some((row, column)))
            .map(|column| scores[row][column] + exhaustive(scores, excluded, row + 1, used | (1 << column)))
            .max()
            .unwrap()
    }
    for seed in 1..=32 {
        let scores = (0..3)
            .map(|row| {
                (0..6)
                    .map(|column| {
                        if column < 3 {
                            (seed * 17 + row * 13 + column * 7) % 31 + 1
                        } else {
                            0
                        }
                    })
                    .collect::<Vec<i128>>()
            })
            .collect::<Vec<_>>();
        let (allocation, total) = assign(&scores, None);
        assert_eq!(total, exhaustive(&scores, None, 0, 0));
        for (row, column) in allocation.into_iter().enumerate() {
            let excluded = Some((row, column));
            assert_eq!(assign(&scores, excluded).1, exhaustive(&scores, excluded, 0, 0));
        }
    }
}
