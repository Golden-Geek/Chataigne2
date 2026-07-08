use std::{fs, time::SystemTime};

use golden_core::{
    app::ensure_preferences_tree,
    edit::{Edit, NodeTree},
    node::{Folder, Node},
    parameter::{ParamValue, Parameter},
};

use super::FormulaCatalog;
use crate::app::{
    state_machine_nodes_formula::{
        external_formula_tree_for_path, FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX,
        FORMULA_EXTERNAL_READ_ONLY_TAG, FORMULA_MANAGED_REGIONS_JSON_DECL_ID,
    },
    AlchemistFormulaDefinition, AppEngine, AppNode, FormulaLibrary,
};

const ACTION_FORMULA: &str = include_str!("../../builtin_formulas/Action.json");

#[test]
fn shipped_builtin_formula_files_load_as_read_only_external_formulas() {
    let engine = engine_with_builtin_formula_library();
    let snapshot = engine.process_tree_snapshot();
    let formulas = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == AlchemistFormulaDefinition::NODE_TYPE)
        .map(|(id, node)| {
            (
                id,
                node.node_data().meta.label.clone(),
                node.node_data().meta.uuid,
                node.node_data().meta.tags.clone(),
                node.node_data().meta.user_permissions.clone(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        formulas
            .iter()
            .map(|(_, label, uuid, _, _)| (label.as_str(), uuid.0.to_string()))
            .collect::<Vec<_>>(),
        vec![
            (
                "Action",
                "11111111-2222-4333-8444-000000000001".to_owned(),
            ),
            (
                "Mapping",
                "11111111-2222-4333-8444-000000000002".to_owned(),
            ),
        ]
    );
    for (_, _, _, tags, permissions) in &formulas {
        assert!(tags.iter().any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG));
        assert!(tags
            .iter()
            .any(|tag| tag.starts_with(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX)));
        assert!(!permissions.can_edit_name);
        assert!(!permissions.can_remove_and_duplicate);
    }
    for (formula_id, label, ..) in &formulas {
        let mut parameter_count = 0usize;
        for (node_id, node) in engine.nodes.iter() {
            let Some(parameter) = node.as_any().downcast_ref::<Parameter>() else {
                continue;
            };
            let mut current = Some(node_id);
            let mut belongs_to_formula = false;
            while let Some(current_id) = current {
                if current_id == *formula_id {
                    belongs_to_formula = true;
                    break;
                }
                current = engine
                    .nodes
                    .get(current_id)
                    .and_then(|ancestor| ancestor.node_data().parent);
            }
            if !belongs_to_formula {
                continue;
            }
            parameter_count += 1;
            assert!(
                parameter.read_only,
                "{label} parameter {} should be read-only",
                node.node_data().meta.label
            );
        }
        assert!(parameter_count > 0, "{label} should have parameters to lock");
    }

    let (action_id, ..) = formulas[0];
    let regions_param = snapshot
        .find_child_by_decl_id(action_id, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
        .expect("Action formula should expose managed region metadata");
    let raw_regions = match snapshot.node(regions_param).and_then(|node| node.param_value.as_ref())
    {
        Some(ParamValue::Str(value)) => value,
        other => panic!("Action managed region metadata should be a string, got {other:?}"),
    };
    let regions: Vec<serde_json::Value> =
        serde_json::from_str(raw_regions).expect("Action managed regions should decode");
    assert_eq!(
        regions
            .iter()
            .map(|region| (
                region["id"].as_str().unwrap_or_default(),
                region["label"].as_str().unwrap_or_default(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("conditions", "Conditions"),
            ("on_true", "On True"),
            ("on_false", "On False"),
        ]
    );
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
fn missing_external_formula_trees_finds_builtins_absent_from_an_older_project() {
    // Simulates a project file saved before "Mapping" existed as a built-in:
    // its Formula Library only has "Action". Re-scanning the shipped
    // built-ins should report "Mapping" as missing, and "Action" as already
    // present (it must not be re-added / duplicated).
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let all_builtins = FormulaCatalog::default_builtin_formula_trees()
        .expect("built-in formulas should load");
    let action_only = all_builtins
        .into_iter()
        .find(|tree| tree.node.node_data().meta.label == "Action")
        .expect("Action built-in should exist");

    let mut library_tree = NodeTree::new(FormulaLibrary::new());
    library_tree.push_child(action_only);
    engine.edits.push(Edit::AddNodeTree {
        tree: library_tree,
        parent: engine.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("formula library should attach");
    }

    let library_id = formula_library_id(&engine);
    let snapshot = engine.process_tree_snapshot();
    let candidates = FormulaCatalog::default_builtin_formula_trees()
        .expect("built-in formulas should load");
    let missing =
        FormulaCatalog::missing_external_formula_trees(&snapshot, library_id, candidates);

    assert_eq!(
        missing
            .iter()
            .map(|tree| tree.node.node_data().meta.label.clone())
            .collect::<Vec<_>>(),
        vec!["Mapping"]
    );
}

#[test]
fn missing_shared_formula_trees_creates_external_file_formulas_for_new_shared_files() {
    // Shared formulas are just external-file-linked formulas pointed at the
    // shared folder, so scanning it should produce a fresh external-file
    // tree per file, and skip files a library child already links to.
    let shared_dir = temp_builtin_formula_dir("shared_merge");
    fs::write(shared_dir.join("MyShared.json"), "placeholder").expect("temp shared file should write");
    fs::write(shared_dir.join("AlreadyLinked.json"), "placeholder").expect("temp shared file should write");

    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let already_linked_path = shared_dir.join("AlreadyLinked.json");
    let mut library_tree = NodeTree::new(FormulaLibrary::new());
    library_tree.push_child(external_formula_tree_for_path(&already_linked_path, "Already Linked"));
    engine.edits.push(Edit::AddNodeTree {
        tree: library_tree,
        parent: engine.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        engine.apply_edits().expect("formula library should attach");
    }

    let library_id = formula_library_id(&engine);
    let snapshot = engine.process_tree_snapshot();
    let missing = FormulaCatalog::missing_shared_formula_trees(&snapshot, library_id, &shared_dir)
        .expect("shared directory should scan");

    assert_eq!(
        missing
            .iter()
            .map(|tree| tree.node.node_data().meta.label.clone())
            .collect::<Vec<_>>(),
        vec!["MyShared"],
        "only the not-yet-linked shared file should be reported as missing"
    );

    fs::remove_dir_all(&shared_dir).ok();
}

#[test]
fn default_missing_shared_formula_trees_uses_preferences_data_folder() {
    let _shared_dir_guard = shared_formula_dir_env_removed();
    let data_folder = temp_builtin_formula_dir("preferences_data_folder");
    let shared_dir = data_folder.join("formulas");
    fs::create_dir_all(&shared_dir).expect("shared formulas folder should be creatable");
    fs::write(shared_dir.join("FromPreferences.json"), "placeholder")
        .expect("temp shared file should write");

    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    ensure_preferences_tree(&mut engine, data_folder.to_string_lossy().to_string());
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(FormulaLibrary::new()),
        parent: engine.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("preferences and formula library should attach");
    }

    let library_id = formula_library_id(&engine);
    let snapshot = engine.process_tree_snapshot();
    let missing = FormulaCatalog::default_missing_shared_formula_trees(&snapshot, library_id)
        .expect("configured shared directory should scan");

    assert_eq!(
        missing
            .iter()
            .map(|tree| tree.node.node_data().meta.label.clone())
            .collect::<Vec<_>>(),
        vec!["FromPreferences"]
    );

    fs::remove_dir_all(&data_folder).ok();
}

#[test]
fn processor_palette_builtin_items_expose_sibling_icon() {
    let catalog = catalog_with_project_formula("Session Formula");
    let palette = catalog.processor_palette_items();

    let action_icon = palette[0]
        .icon
        .as_deref()
        .expect("Action processor palette item should have an icon");
    assert!(action_icon.starts_with("data:image/svg+xml;base64,"));

    let mapping_icon = palette[1]
        .icon
        .as_deref()
        .expect("Mapping processor palette item should have an icon");
    assert!(mapping_icon.starts_with("data:image/svg+xml;base64,"));

    let session_index = palette
        .iter()
        .position(|item| item.label == "Session Formula")
        .expect("session formula should be in the processor palette");
    assert!(
        palette[session_index].icon.is_none(),
        "Session formula without a sibling icon file should have no palette icon"
    );
}

#[test]
fn processor_palette_groups_builtin_shared_and_project_formulas() {
    let catalog =
        catalog_with_shared_and_project_formulas("Shared Formula", "Project Formula");
    let palette = catalog.processor_palette_items();

    assert_eq!(
        palette.iter().map(|item| item.label.as_str()).collect::<Vec<_>>(),
        vec!["Action", "Mapping", "Shared Formula", "Project Formula"]
    );
    assert!(!palette[0].separator_before, "Action starts the built-in group");
    assert!(!palette[1].separator_before, "Mapping stays in the built-in group");
    assert!(
        palette[2].separator_before,
        "Shared Formula should start a new group after built-ins"
    );
    assert!(
        palette[3].separator_before,
        "Project Formula should start a new group after shared formulas"
    );
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

    let trees = FormulaCatalog::builtin_formula_trees(&dir)
        .expect("re-exported Action formula should load");
    let action = trees
        .first()
        .expect("Action tree should be loaded from the temp built-in dir");

    assert_eq!(
        action.node.node_data().meta.uuid.0.to_string(),
        "11111111-2222-4333-8444-000000000001"
    );
    assert_eq!(action.node.node_data().meta.label, "Action Reexport");
    assert!(action
        .node
        .node_data()
        .meta
        .tags
        .iter()
        .any(|tag| tag == "chataigne.formula.external.builtin:chataigne.action@1"));

    fs::remove_dir_all(dir).expect("temp built-in formula dir should be removable");
}

#[test]
fn builtin_formula_with_sibling_svg_gets_icon_presentation() {
    let action: serde_json::Value =
        serde_json::from_str(ACTION_FORMULA).expect("Action export should be JSON");

    let dir = temp_builtin_formula_dir("action_icon_svg");
    fs::write(
        dir.join("Action.json"),
        serde_json::to_string_pretty(&action).expect("Action export should encode"),
    )
    .expect("temp Action export should be writable");
    fs::write(
        dir.join("Action.svg"),
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
    )
    .expect("temp Action icon should be writable");

    let trees = FormulaCatalog::builtin_formula_trees(&dir)
        .expect("Action formula with sibling icon should load");
    let action = trees
        .first()
        .expect("Action tree should be loaded from the temp built-in dir");

    let icon = action
        .node
        .node_data()
        .meta
        .presentation
        .icon
        .as_deref()
        .expect("Action formula should have an icon from its sibling .svg file");
    assert!(
        icon.starts_with("data:image/svg+xml;base64,"),
        "Action icon should be an svg data URI, got {icon}"
    );

    fs::remove_dir_all(dir).expect("temp built-in formula dir should be removable");
}

#[test]
fn builtin_formula_without_sibling_icon_has_no_icon_presentation() {
    let action: serde_json::Value =
        serde_json::from_str(ACTION_FORMULA).expect("Action export should be JSON");

    let dir = temp_builtin_formula_dir("action_icon_missing");
    fs::write(
        dir.join("Action.json"),
        serde_json::to_string_pretty(&action).expect("Action export should encode"),
    )
    .expect("temp Action export should be writable");

    let trees = FormulaCatalog::builtin_formula_trees(&dir)
        .expect("Action formula without sibling icon should load");
    let action = trees
        .first()
        .expect("Action tree should be loaded from the temp built-in dir");

    assert!(
        action.node.node_data().meta.presentation.icon.is_none(),
        "Action formula should have no icon without a sibling image file"
    );

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
    let mut engine = engine_with_builtin_formula_library();

    let library_id = formula_library_id(&engine);
    let mut formula = AlchemistFormulaDefinition::new();
    formula.node_data_mut().meta.label = label.to_owned();
    engine.add_user_item(formula.into(), Some(library_id));
    engine
        .apply_edits()
        .expect("project formula should attach");

    FormulaCatalog::from_snapshot(&engine.process_tree_snapshot())
}

fn catalog_with_shared_and_project_formulas(
    shared_label: &str,
    project_label: &str,
) -> FormulaCatalog {
    // Shared formulas are classified by their linked file living inside the
    // shared formulas folder, so point one there (the file itself doesn't
    // need to exist for classification/palette-grouping purposes).
    let _shared_dir_guard = shared_formula_dir_test_override();
    let shared_dir = std::env::var_os("CHATAIGNE_SHARED_FORMULAS_DIR")
        .expect("guard should set CHATAIGNE_SHARED_FORMULAS_DIR");
    let shared_path = std::path::Path::new(&shared_dir).join("shared_formula.json");

    let mut engine = engine_with_builtin_formula_library();
    let library_id = formula_library_id(&engine);

    let shared_tree = external_formula_tree_for_path(&shared_path, shared_label);
    engine.edits.push(Edit::AddNodeTree {
        tree: shared_tree,
        parent: library_id,
        prev_sibling: None,
    });

    let mut project_formula = AlchemistFormulaDefinition::new();
    project_formula.node_data_mut().meta.label = project_label.to_owned();
    engine.add_user_item(project_formula.into(), Some(library_id));

    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("shared and project formulas should attach");
    }

    FormulaCatalog::from_snapshot(&engine.process_tree_snapshot())
}

/// Points CHATAIGNE_SHARED_FORMULAS_DIR at a directory that doesn't exist,
/// so shared-formula resolution sees zero shared formulas regardless of
/// what the current machine's real Shared formulas folder contains, and
/// tests that need to build a path "inside" the shared folder have a known
/// value to build against. Restores the previous value (if any) when dropped.
struct SharedFormulaDirTestOverride {
    previous: Option<std::ffi::OsString>,
}

impl Drop for SharedFormulaDirTestOverride {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var("CHATAIGNE_SHARED_FORMULAS_DIR", value) },
            None => unsafe { std::env::remove_var("CHATAIGNE_SHARED_FORMULAS_DIR") },
        }
    }
}

fn shared_formula_dir_test_override() -> SharedFormulaDirTestOverride {
    let previous = std::env::var_os("CHATAIGNE_SHARED_FORMULAS_DIR");
    unsafe {
        std::env::set_var(
            "CHATAIGNE_SHARED_FORMULAS_DIR",
            "chataigne2-tests-nonexistent-shared-formulas-dir",
        );
    }
    SharedFormulaDirTestOverride { previous }
}

fn shared_formula_dir_env_removed() -> SharedFormulaDirTestOverride {
    let previous = std::env::var_os("CHATAIGNE_SHARED_FORMULAS_DIR");
    unsafe {
        std::env::remove_var("CHATAIGNE_SHARED_FORMULAS_DIR");
    }
    SharedFormulaDirTestOverride { previous }
}

fn engine_with_builtin_formula_library() -> AppEngine {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let mut library_tree = NodeTree::new(FormulaLibrary::new());
    for formula in FormulaCatalog::default_builtin_formula_trees()
        .expect("built-in formulas should load")
    {
        library_tree.push_child(formula);
    }
    engine.edits.push(Edit::AddNodeTree {
        tree: library_tree,
        parent: engine.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("built-in formula library should attach");
    }
    engine
}

fn formula_library_id(engine: &AppEngine) -> golden_core::node::NodeId {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("formula library should exist")
}
