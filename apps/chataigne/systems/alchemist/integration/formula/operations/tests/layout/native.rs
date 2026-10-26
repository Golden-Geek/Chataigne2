use super::*;
use std::{fs, path::PathBuf, process::Command};

#[test]
#[ignore = "requires current default cargo build; owned headless app and production browser fixture"]
fn formula_graph_native_browser_adapter() {
    let (mut engine, formula) = fixture();
    let states = [
        anode(&mut engine, &formula, "constant"),
        anode(&mut engine, &formula, "math"),
    ];
    for (index, node) in states.into_iter().enumerate() {
        let id = identity(&engine, node);
        edit(
            &mut engine,
            &formula,
            FormulaGraphEdit::Move {
                moves: vec![FormulaGraphMove {
                    anode: id.clone(),
                    position: [2. + index as f64 * 24., 3.],
                }],
            },
        )
        .unwrap();
        edit(
            &mut engine,
            &formula,
            FormulaGraphEdit::Rename {
                anode: id,
                label: if index == 0 { "First" } else { "Second" }.into(),
            },
        )
        .unwrap();
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let evidence = root
        .join("target/qualification")
        .join(format!("p11-alchemist-native-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&evidence).unwrap();
    fs::write(
        evidence.join("fixture.noisette"),
        engine.to_project_json_with(Node::project_encode_data).unwrap(),
    )
    .unwrap();
    let position = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(states[1], "position")
        .unwrap();
    let position = engine
        .nodes
        .get_mut(position)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap();
    position.read_only = true;
    position.persist_read_only_value = true;
    fs::write(
        evidence.join("readonly.noisette"),
        engine.to_project_json_with(Node::project_encode_data).unwrap(),
    )
    .unwrap();
    let mut command = Command::new("node");
    command
        .current_dir(&root)
        .arg(root.join("apps/chataigne/ui/tests/browser/alchemist-graph-native.mjs"))
        .arg(root.join(format!("target/debug/Chataigne2{}", std::env::consts::EXE_SUFFIX)))
        .arg(&evidence);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().unwrap();
    eprintln!("Native Alchemist graph evidence: {}", evidence.display());
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
