use super::*;
use std::{fs, path::PathBuf, process::Command};

#[test]
#[ignore = "requires current default cargo build; owned headless app and production browser fixture"]
fn state_graph_authoring_native_browser_adapter() {
    let (mut engine, manager, states) = fixture();
    assert!(
        invoke(
            &mut engine,
            manager,
            StateGraphEdit::Move {
                moves: states
                    .into_iter()
                    .enumerate()
                    .map(|(index, state)| StateGraphMove {
                        state,
                        position: [2.0 + index as f64 * 24.0, 3.0],
                    })
                    .collect()
            }
        )
        .success
    );
    for (state, label) in states.into_iter().zip(["First", "Second"]) {
        assert!(
            invoke(
                &mut engine,
                manager,
                StateGraphEdit::Rename {
                    state,
                    label: label.into()
                }
            )
            .success
        );
    }
    // Node's entrypoint resolver rejects Windows verbatim (\\?\) paths.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let evidence = root
        .join("target/qualification")
        .join(format!("p11-state-native-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&evidence).unwrap();
    fs::write(
        evidence.join("fixture.noisette"),
        engine.to_project_json_with(Node::project_encode_data).unwrap(),
    )
    .unwrap();
    let position = child(&engine, states[1].node, "position").unwrap();
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
        .arg(root.join("apps/chataigne/ui/tests/browser/state-graph-native.mjs"))
        .arg(root.join(format!("target/debug/Chataigne2{}", std::env::consts::EXE_SUFFIX)))
        .arg(&evidence);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().unwrap();
    eprintln!("Native state graph evidence: {}", evidence.display());
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
