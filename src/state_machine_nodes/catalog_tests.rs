use std::{fs, time::SystemTime};

use golden_core::node::{Folder, Node};

use super::{FormulaCatalog, FormulaSourceRef};

const ACTION_FORMULA: &str = include_str!("../../builtin_formulas/Action.json");

#[test]
fn shipped_builtin_formula_files_load_action_and_mapping_with_stable_sources() {
    let catalog = FormulaCatalog::from_builtin_formula_dir("builtin_formulas")
        .expect("shipped built-in formula files should load");

    let palette: Vec<_> = catalog
        .processor_palette_items()
        .into_iter()
        .map(|item| {
            (
                item.label,
                item.node_type,
                item.menu_path,
                item.separator_before,
            )
        })
        .collect();

    assert_eq!(
        palette,
        vec![
            (
                "Action".to_owned(),
                "state_processor:builtin:chataigne.action@1".to_owned(),
                Vec::<String>::new(),
                false,
            ),
            (
                "Mapping".to_owned(),
                "state_processor:builtin:chataigne.mapping@1".to_owned(),
                Vec::<String>::new(),
                false,
            ),
        ]
    );

    let action = catalog
        .resolve_builtin(&FormulaSourceRef::builtin("chataigne", "action", 1))
        .expect("Action formula should resolve");
    assert_eq!(action.id.as_str(), "chataigne.action");
    assert_eq!(action.label, "Action");

    let mapping = catalog
        .resolve_builtin(&FormulaSourceRef::builtin("chataigne", "mapping", 1))
        .expect("Mapping formula should resolve");
    assert_eq!(mapping.id.as_str(), "chataigne.mapping");
    assert_eq!(mapping.label, "Mapping");
}

#[test]
fn processor_palette_places_session_formulas_after_builtin_separator() {
    let catalog = catalog_with_project_formula("Session Formula");
    let palette = catalog.processor_palette_items();

    let first_session_index = palette
        .iter()
        .position(|item| item.label == "Session Formula")
        .expect("session formula should be in the processor palette");

    assert_eq!(palette[0].label, "Action");
    assert_eq!(palette[1].label, "Mapping");
    assert_eq!(first_session_index, 2);
    assert!(palette.iter().all(|item| item.menu_path.is_empty()));
    assert!(!palette[0].separator_before);
    assert!(!palette[1].separator_before);
    assert!(palette[first_session_index].separator_before);
}

#[test]
fn exported_action_file_name_forces_stable_builtin_identity() {
    let mut action: serde_json::Value =
        serde_json::from_str(ACTION_FORMULA).expect("Action export should be JSON");
    let root = action
        .get_mut("nodes")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|nodes| nodes.first_mut())
        .expect("Action export should contain a formula root");
    root["sourceUuid"] = serde_json::json!("11111111-1111-1111-1111-111111111111");
    root["label"] = serde_json::json!("Action Reexport");
    root["meta"]["label"] = serde_json::json!("Action Reexport");

    let dir = temp_builtin_formula_dir("action_identity");
    fs::write(
        dir.join("Action.json"),
        serde_json::to_string_pretty(&action).expect("mutated Action export should encode"),
    )
    .expect("temp Action export should be writable");

    let catalog = FormulaCatalog::from_builtin_formula_dir(&dir)
        .expect("re-exported Action formula should load");
    let action = catalog
        .resolve_builtin(&FormulaSourceRef::builtin("chataigne", "action", 1))
        .expect("Action source should stay stable after re-export");

    assert_eq!(action.id.as_str(), "chataigne.action");
    assert_eq!(action.label, "Action Reexport");

    fs::remove_dir_all(dir).expect("temp built-in formula dir should be removable");
}

fn temp_builtin_formula_dir(name: &str) -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "chataigne2_builtin_formula_{name}_{}_{}",
        std::process::id(),
        timestamp
    ));
    fs::create_dir_all(&dir).expect("temp built-in formula dir should be creatable");
    dir
}

fn catalog_with_project_formula(label: &str) -> FormulaCatalog {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(crate::app::FormulaLibrary::new().into(), None);
    engine
        .apply_edits()
        .expect("formula library should attach");

    let library_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == crate::app::FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("formula library should exist");

    let mut formula = crate::app::AlchemistFormulaDefinition::new();
    formula.node_data_mut().meta.label = label.to_owned();
    engine.add_user_item(formula.into(), Some(library_id));
    engine
        .apply_edits()
        .expect("project formula should attach");

    FormulaCatalog::from_snapshot(&engine.process_tree_snapshot())
}
