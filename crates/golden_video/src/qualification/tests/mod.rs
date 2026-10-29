use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde_json::{Value, json};

use super::protocol::{
    QualificationAction, QualificationCommand, QualificationLayout, QualificationPhase, QualificationReport,
    QualificationView, export_typescript,
};
use crate::preview::protocol::{CssRect, PreviewView};

fn files(directory: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"),
            )
        })
        .collect()
}

#[test]
fn committed_qualification_typescript_matches_rust_including_the_exact_file_set() {
    let temporary = tempfile::tempdir().unwrap();
    export_typescript(temporary.path()).unwrap();
    let generated = files(temporary.path());
    let expected = [
        "CssRect.ts",
        "PreviewView.ts",
        "QualificationAction.ts",
        "QualificationCommand.ts",
        "QualificationLayout.ts",
        "QualificationPhase.ts",
        "QualificationReport.ts",
        "QualificationView.ts",
        "constants.ts",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    assert_eq!(generated.keys().cloned().collect::<BTreeSet<_>>(), expected);
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/golden-video-ui/tests/native/generated");
    assert_eq!(generated, files(&committed));
}

fn layout() -> QualificationLayout {
    QualificationLayout {
        revision: 7,
        views: [
            QualificationView {
                source: "opaque-a".into(),
                css_width: 256.,
                css_height: 144.,
            },
            QualificationView {
                source: "opaque-b".into(),
                css_width: 192.,
                css_height: 108.,
            },
        ],
    }
}

#[test]
fn fixture_layout_requires_two_views_and_uses_canonical_camel_case_dimensions() {
    let layout = layout();
    let serialized = serde_json::to_value(&layout).unwrap();
    assert_eq!(
        serialized,
        json!({"revision":7,"views":[
            {"source":"opaque-a","cssWidth":256.,"cssHeight":144.},
            {"source":"opaque-b","cssWidth":192.,"cssHeight":108.},
        ]})
    );
    assert_eq!(
        serde_json::from_value::<QualificationLayout>(serialized.clone()).unwrap(),
        layout
    );
    for count in [0, 1, 3] {
        let mut invalid = serialized.clone();
        invalid["views"] = Value::Array(vec![serialized["views"][0].clone(); count]);
        assert!(
            serde_json::from_value::<QualificationLayout>(invalid).is_err(),
            "accepted {count} views"
        );
    }
    let mut snake_case = serialized;
    let first = snake_case["views"][0].as_object_mut().unwrap();
    let width = first.remove("cssWidth").unwrap();
    first.insert("css_width".into(), width);
    assert!(serde_json::from_value::<QualificationLayout>(snake_case).is_err());
}

#[test]
fn fixture_actions_and_reports_keep_their_generated_wire_shape() {
    for (action, name) in [
        (QualificationAction::Ready, "ready"),
        (QualificationAction::Hide, "hide"),
        (QualificationAction::Show, "show"),
        (QualificationAction::Disconnect, "disconnect"),
    ] {
        let command = QualificationCommand { id: 11, action };
        let serialized = serde_json::to_value(command).unwrap();
        assert_eq!(serialized, json!({"id":11,"action":name}));
        assert_eq!(
            serde_json::from_value::<QualificationCommand>(serialized).unwrap(),
            command
        );
    }
    assert!(serde_json::from_value::<QualificationCommand>(json!({"id":11,"action":"Ready"})).is_err());
    let layout = layout();
    let views = layout
        .views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let rect = CssRect {
                x: 0.,
                y: 0.,
                width: view.css_width,
                height: view.css_height,
            };
            PreviewView {
                id: uuid::Uuid::from_u128(index as u128 + 1),
                source: view.source.clone(),
                rect,
                clip: rect,
                visible: true,
            }
        })
        .collect();
    let report = QualificationReport {
        sequence: 3,
        command_id: Some(11),
        phase: QualificationPhase::Ready,
        layout: Some(layout),
        installed: true,
        visible_views: 2,
        views,
        retirement_masks: 0,
        max_retirement_masks: 2,
        overlay_clicks: [1, 2],
        error: None,
    };
    let serialized = serde_json::to_value(&report).unwrap();
    let expected = [
        "sequence",
        "commandId",
        "phase",
        "layout",
        "installed",
        "visibleViews",
        "views",
        "retirementMasks",
        "maxRetirementMasks",
        "overlayClicks",
        "error",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(
        serialized
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(serialized["phase"], "ready");
    assert_eq!(serialized["views"].as_array().unwrap().len(), 2);
    assert_eq!(
        serde_json::from_value::<QualificationReport>(serialized.clone()).unwrap(),
        report
    );
    let mut invalid = serialized;
    invalid["overlayClicks"] = json!([1, 2, 3]);
    assert!(serde_json::from_value::<QualificationReport>(invalid).is_err());
}
