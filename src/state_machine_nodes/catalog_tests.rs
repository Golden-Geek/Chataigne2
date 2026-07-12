use std::{
    collections::HashSet,
    fs,
    time::SystemTime,
};

use golden_core::{
    app::ensure_preferences_tree,
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeMetaPatch},
    parameter::{ParamValue, Parameter},
    process_ctx::ExecutionPhase,
    ui_sync::UiEditIntent,
};

use super::FormulaCatalog;
use crate::app::{
    state_machine_nodes_formula::{
        create_anode_user_item_tree, external_formula_tree_for_path,
        ANODE_CREATE_PREFIX, FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX,
        FORMULA_EXTERNAL_FILE_DECL_ID, FORMULA_EXTERNAL_READ_ONLY_TAG,
        FORMULA_MANAGED_REGIONS_JSON_DECL_ID,
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
                "22930516-b6bb-56de-9914-d55cd7f6f094".to_owned(),
            ),
            (
                "Mapping",
                "9b0a8eda-dcb7-584d-8cad-b47f151c3444".to_owned(),
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

    for (formula_id, label, ..) in &formulas {
        let regions_param = snapshot
            .find_child_by_decl_id(*formula_id, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
            .unwrap_or_else(|| panic!("{label} formula should expose managed region metadata"));
        assert_eq!(
            snapshot.node(regions_param).and_then(|node| node.param_value.as_ref()),
            Some(&ParamValue::Str(String::new())),
            "{label} should not ship generated managed-region runtime metadata"
        );
    }
}

#[test]
fn exported_formula_json_uses_tagged_parameter_values() {
    let engine = engine_with_builtin_formula_library();
    let action_id = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
                && node.node_data().meta.label == "Action"
        })
        .map(|(id, _)| id)
        .expect("Action formula should exist");
    let snapshot = engine.process_tree_snapshot();
    let export = FormulaCatalog::export_formula_json(&snapshot, action_id)
        .expect("Action formula should export");
    let export: serde_json::Value =
        serde_json::from_str(&export).expect("formula export should be JSON");
    let root = export
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .and_then(|nodes| nodes.first())
        .expect("formula export should contain a root node");

    let managed_regions =
        exported_node_by_decl_id(root, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
            .expect("managed region metadata should be exported");
    assert_eq!(managed_regions["data"]["param"]["value"]["kind"], "str");
    assert_eq!(managed_regions["data"]["param"]["value"]["value"], "");

    let role = exported_node_by_decl_id(root, "role").expect("role should be exported");
    assert_eq!(role["data"]["param"]["value"]["kind"], "enum");
}

#[test]
fn exported_formula_json_preserves_anode_internal_type_tags() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let routing_type = chataigne_state_machine::alchemist::ROUTING_TYPE;

    let mut formula = AlchemistFormulaDefinition::new();
    formula.node_data_mut().meta.label = "Shared Source".to_owned();
    let mut formula_tree = NodeTree::new(formula);
    formula_tree.push_child(
        create_anode_user_item_tree(&format!("{ANODE_CREATE_PREFIX}{routing_type}"))
            .expect("routing ANode should be creatable"),
    );
    engine.edits.push(Edit::AddNodeTree {
        tree: formula_tree,
        parent: engine.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("formula tree should attach");
    }

    let formula_id = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.get_type() == AlchemistFormulaDefinition::NODE_TYPE
                && node.node_data().meta.label == "Shared Source"
        })
        .map(|(id, _)| id)
        .expect("shared source formula should exist");
    let snapshot = engine.process_tree_snapshot();
    let export = FormulaCatalog::export_formula_json(&snapshot, formula_id)
        .expect("formula should export");
    let export: serde_json::Value =
        serde_json::from_str(&export).expect("formula export should be JSON");
    let root = export
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .and_then(|nodes| nodes.first())
        .expect("formula export should contain a root node");
    let anode =
        exported_node_by_type(root, "alchemist_anode").expect("ANode should be exported");
    let tags = anode["meta"]["tags"]
        .as_array()
        .expect("ANode metadata should include tags");
    let expected_tag = format!("alchemist.anode.type:{routing_type}");
    assert!(
        tags.iter()
            .any(|tag| tag.as_str() == Some(expected_tag.as_str())),
        "ANode export should preserve its internal type tag"
    );
}

#[test]
fn exported_builtin_formula_import_accepts_legacy_untagged_param_values() {
    let mut action: serde_json::Value =
        serde_json::from_str(ACTION_FORMULA).expect("Action export should be JSON");
    let root = action
        .get_mut("nodes")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|nodes| nodes.first_mut())
        .expect("Action export should contain a formula root");

    let managed_regions =
        exported_node_by_decl_id_mut(root, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
            .expect("managed region metadata should be exported");
    let managed_regions_value = managed_regions["data"]["param"]["value"]["value"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    managed_regions["data"]["param"]["value"] = serde_json::json!(managed_regions_value);

    let role = exported_node_by_decl_id_mut(root, "role").expect("role should be exported");
    let role_value = role["data"]["param"]["value"]["value"]
        .as_str()
        .expect("role should have a string value")
        .to_owned();
    role["data"]["param"]["value"] = serde_json::json!(role_value);

    let dir = temp_builtin_formula_dir("legacy_untagged_param_values");
    fs::write(
        dir.join("Action.json"),
        serde_json::to_string_pretty(&action).expect("mutated Action export should encode"),
    )
    .expect("temp Action export should be writable");

    let trees = FormulaCatalog::builtin_formula_trees(&dir)
        .expect("Action formula with legacy untagged values should load");

    assert_eq!(trees.len(), 1);
    fs::remove_dir_all(&dir).ok();
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
fn stale_shared_formula_nodes_reports_links_whose_files_were_removed() {
    let shared_dir = temp_builtin_formula_dir("shared_stale");
    let removed_path = shared_dir.join("Removed.json");
    let kept_path = shared_dir.join("Kept.json");
    fs::write(&kept_path, "placeholder").expect("kept shared file should write");

    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let mut library_tree = NodeTree::new(FormulaLibrary::new());
    library_tree.push_child(external_formula_tree_for_path(&removed_path, "Removed"));
    library_tree.push_child(external_formula_tree_for_path(&kept_path, "Kept"));
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
    let stale = FormulaCatalog::stale_shared_formula_nodes(&snapshot, library_id, &shared_dir)
        .expect("shared directory should scan");
    let stale_labels = stale
        .into_iter()
        .filter_map(|node| snapshot.node(node).map(|node| node.label.clone()))
        .collect::<Vec<_>>();

    assert_eq!(stale_labels, vec!["Removed"]);

    fs::remove_dir_all(&shared_dir).ok();
}

#[test]
fn renaming_shared_formula_moves_file_and_keeps_formula_uuid() {
    let shared_dir = temp_builtin_formula_dir("shared_rename");
    let _shared_dir_guard = shared_formula_dir_test_override_to(&shared_dir);
    let old_path = shared_dir.join("Old Shared.json");
    let new_path = shared_dir.join("New Shared.json");

    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let mut library_tree = NodeTree::new(FormulaLibrary::new());
    library_tree.push_child(external_formula_tree_for_path(&old_path, "Old Shared"));
    engine.edits.push(Edit::AddNodeTree {
        tree: library_tree,
        parent: engine.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        engine.apply_edits().expect("formula library should attach");
    }

    let library_id = formula_library_id(&engine);
    let formula_id = only_formula_child(&engine, library_id);
    let snapshot = engine.process_tree_snapshot();
    let formula_uuid = snapshot
        .node(formula_id)
        .expect("shared formula should exist")
        .uuid;
    let exported = FormulaCatalog::export_formula_json(&snapshot, formula_id)
        .expect("shared formula should export");
    fs::write(&old_path, exported).expect("old shared formula file should write");

    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: formula_id,
        patch: NodeMetaPatch {
            label: Some("New Shared".to_owned()),
            ..Default::default()
        },
    });
    assert!(ack.success, "shared formula rename should succeed: {ack:?}");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("shared formula should receive the rename");
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("shared formula rename should settle");
    }

    let snapshot = engine.process_tree_snapshot();
    assert_eq!(
        snapshot
            .node(formula_id)
            .expect("shared formula should still exist")
            .uuid,
        formula_uuid,
        "renaming the shared formula must not replace the linked node"
    );
    assert!(
        !old_path.exists(),
        "renaming the shared formula should move the old file"
    );
    assert!(
        new_path.exists(),
        "renaming the shared formula should create the renamed file"
    );
    assert_eq!(
        shared_formula_file_param(&snapshot, formula_id),
        new_path,
        "the existing formula node should now link to the renamed file"
    );

    fs::remove_dir_all(&shared_dir).ok();
}

#[test]
fn renamed_shared_formula_file_relinks_existing_node_by_uuid() {
    let shared_dir = temp_builtin_formula_dir("shared_relink");
    let old_path = shared_dir.join("Old Shared.json");
    let new_path = shared_dir.join("New Shared.json");

    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    let mut library_tree = NodeTree::new(FormulaLibrary::new());
    library_tree.push_child(external_formula_tree_for_path(&old_path, "Old Shared"));
    engine.edits.push(Edit::AddNodeTree {
        tree: library_tree,
        parent: engine.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        engine.apply_edits().expect("formula library should attach");
    }

    let library_id = formula_library_id(&engine);
    let formula_id = only_formula_child(&engine, library_id);
    let snapshot = engine.process_tree_snapshot();
    let exported = FormulaCatalog::export_formula_json(&snapshot, formula_id)
        .expect("shared formula should export");
    let mut exported: serde_json::Value =
        serde_json::from_str(&exported).expect("shared formula export should decode");
    exported["nodes"][0]["label"] = serde_json::json!("New Shared");
    exported["nodes"][0]["meta"]["label"] = serde_json::json!("New Shared");
    let exported =
        serde_json::to_string_pretty(&exported).expect("renamed shared formula should encode");
    fs::write(&new_path, exported).expect("renamed shared formula file should write");

    let renamed = FormulaCatalog::renamed_shared_formula_paths(&snapshot, library_id, &shared_dir)
        .expect("shared directory should scan");
    assert_eq!(renamed.len(), 1);
    assert_eq!(renamed[0].node, formula_id);
    assert_eq!(renamed[0].path, new_path);
    assert_eq!(
        renamed[0].label, "New Shared",
        "the renamed file label should be carried into the existing session"
    );

    let renamed_nodes = renamed
        .iter()
        .map(|rename| rename.node)
        .collect::<HashSet<_>>();
    let renamed_paths = renamed
        .iter()
        .map(|rename| rename.path.clone())
        .collect::<HashSet<_>>();
    assert!(
        FormulaCatalog::stale_shared_formula_nodes_excluding(
            &snapshot,
            library_id,
            &shared_dir,
            &renamed_nodes,
        )
        .expect("shared directory should scan")
        .is_empty(),
        "a renamed shared formula must not be removed from existing sessions"
    );
    assert!(
        FormulaCatalog::missing_shared_formula_trees_excluding_paths(
            &snapshot,
            library_id,
            &shared_dir,
            &renamed_paths,
        )
        .expect("shared directory should scan")
        .is_empty(),
        "a renamed shared formula must not be re-added with a new UUID"
    );

    fs::remove_dir_all(&shared_dir).ok();
}

#[test]
fn shared_formula_renamed_from_another_session_preserves_file_uuid() {
    let shared_dir = temp_builtin_formula_dir("shared_cross_session_rename");
    let _shared_dir_guard = shared_formula_dir_test_override_to(&shared_dir);
    let old_path = shared_dir.join("Old Shared.json");
    let new_path = shared_dir.join("New Shared.json");

    let root: AppNode = Folder::new("root").into();
    let mut first_session = AppEngine::new(root);
    let mut first_library_tree = NodeTree::new(FormulaLibrary::new());
    first_library_tree.push_child(external_formula_tree_for_path(&old_path, "Old Shared"));
    first_session.edits.push(Edit::AddNodeTree {
        tree: first_library_tree,
        parent: first_session.root,
        prev_sibling: None,
    });
    for _ in 0..4 {
        first_session
            .apply_edits()
            .expect("first session formula library should attach");
    }
    let first_library_id = formula_library_id(&first_session);
    let first_formula_id = only_formula_child(&first_session, first_library_id);
    let first_snapshot = first_session.process_tree_snapshot();
    let original_uuid = first_snapshot
        .node(first_formula_id)
        .expect("first session shared formula should exist")
        .uuid;
    let exported = FormulaCatalog::export_formula_json(&first_snapshot, first_formula_id)
        .expect("shared formula should export");
    fs::write(&old_path, exported).expect("original shared formula file should write");

    let root: AppNode = Folder::new("root").into();
    let mut second_session = AppEngine::new(root);
    second_session.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(FormulaLibrary::new()),
        parent: second_session.root,
        prev_sibling: None,
    });
    second_session
        .apply_edits()
        .expect("second session formula library should attach");
    let second_library_id = formula_library_id(&second_session);
    let second_snapshot = second_session.process_tree_snapshot();
    let missing =
        FormulaCatalog::missing_shared_formula_trees(&second_snapshot, second_library_id, &shared_dir)
            .expect("shared directory should scan");
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0].node.node_data().meta.uuid, original_uuid,
        "auto-populated shared formulas must use the file's UUID"
    );
    second_session.edits.push(Edit::AddNodeTree {
        tree: missing.into_iter().next().expect("checked length"),
        parent: second_library_id,
        prev_sibling: None,
    });
    for _ in 0..4 {
        second_session
            .apply_edits()
            .expect("second session shared formula should settle");
    }
    let second_formula_id = only_formula_child(&second_session, second_library_id);

    let ack = second_session.apply_ui_intent(UiEditIntent::PatchMeta {
        node: second_formula_id,
        patch: NodeMetaPatch {
            label: Some("New Shared".to_owned()),
            ..Default::default()
        },
    });
    assert!(
        ack.success,
        "second session shared formula rename should succeed: {ack:?}"
    );
    second_session
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("second session should receive the rename");
    for _ in 0..4 {
        second_session
            .apply_edits()
            .expect("second session shared formula rename should settle");
    }

    assert!(!old_path.exists(), "old shared formula path should be moved");
    assert!(new_path.exists(), "renamed shared formula file should exist");
    assert_eq!(
        FormulaCatalog::shared_formula_file_identity(&new_path)
            .expect("renamed shared formula should decode")
            .0,
        original_uuid,
        "renaming from another session must not rewrite the shared file UUID"
    );
    let relink =
        FormulaCatalog::renamed_shared_formula_paths(&first_snapshot, first_library_id, &shared_dir)
            .expect("first session should scan renamed shared formulas");
    assert_eq!(relink.len(), 1);
    assert_eq!(relink[0].node, first_formula_id);
    assert_eq!(relink[0].path, new_path);

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
fn exported_action_file_name_derives_builtin_identity_from_file_stem() {
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
        "22930516-b6bb-56de-9914-d55cd7f6f094"
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
fn shared_formula_dir_test_override() -> crate::test_support::ScopedSharedFormulaDir {
    crate::test_support::scoped_shared_formula_dir(Some(std::path::Path::new(
        "chataigne2-tests-nonexistent-shared-formulas-dir",
    )))
}

fn shared_formula_dir_test_override_to(
    path: &std::path::Path,
) -> crate::test_support::ScopedSharedFormulaDir {
    crate::test_support::scoped_shared_formula_dir(Some(path))
}

fn shared_formula_dir_env_removed() -> crate::test_support::ScopedSharedFormulaDir {
    crate::test_support::scoped_shared_formula_dir(None)
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
    for _ in 0..8 {
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

fn only_formula_child(
    engine: &AppEngine,
    library: golden_core::node::NodeId,
) -> golden_core::node::NodeId {
    let snapshot = engine.process_tree_snapshot();
    snapshot
        .child_ids(library)
        .into_iter()
        .find(|node| {
            snapshot.node(*node).is_some_and(|node| {
                node.node_type == AlchemistFormulaDefinition::NODE_TYPE
            })
        })
        .expect("formula child should exist")
}

fn shared_formula_file_param(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    formula: golden_core::node::NodeId,
) -> std::path::PathBuf {
    let file_param = snapshot
        .find_child_by_decl_id(formula, FORMULA_EXTERNAL_FILE_DECL_ID)
        .expect("shared formula should have a file parameter");
    match snapshot
        .node(file_param)
        .and_then(|node| node.param_value.as_ref())
    {
        Some(ParamValue::File(path)) | Some(ParamValue::Str(path)) => {
            std::path::PathBuf::from(path)
        }
        other => panic!("shared formula file parameter should be a path, got {other:?}"),
    }
}

fn exported_node_by_decl_id<'a>(
    node: &'a serde_json::Value,
    decl_id: &str,
) -> Option<&'a serde_json::Value> {
    if node.get("decl_id").and_then(serde_json::Value::as_str) == Some(decl_id) {
        return Some(node);
    }
    for child in node.get("children")?.as_array()? {
        if let Some(found) = exported_node_by_decl_id(child, decl_id) {
            return Some(found);
        }
    }
    None
}

fn exported_node_by_type<'a>(
    node: &'a serde_json::Value,
    node_type: &str,
) -> Option<&'a serde_json::Value> {
    if node.get("node_type").and_then(serde_json::Value::as_str) == Some(node_type) {
        return Some(node);
    }
    for child in node.get("children")?.as_array()? {
        if let Some(found) = exported_node_by_type(child, node_type) {
            return Some(found);
        }
    }
    None
}

fn exported_node_by_decl_id_mut<'a>(
    node: &'a mut serde_json::Value,
    decl_id: &str,
) -> Option<&'a mut serde_json::Value> {
    if node.get("decl_id").and_then(serde_json::Value::as_str) == Some(decl_id) {
        return Some(node);
    }
    for child in node.get_mut("children")?.as_array_mut()? {
        if let Some(found) = exported_node_by_decl_id_mut(child, decl_id) {
            return Some(found);
        }
    }
    None
}
