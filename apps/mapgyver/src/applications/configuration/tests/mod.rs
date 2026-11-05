use super::*;
use crate::applications::protocol::*;
use std::hash::{Hash, Hasher};

fn definition() -> ApplicationDefinition {
    ApplicationDefinition {
        executable: Some(std::env::current_exe().unwrap().to_str().unwrap().into()),
        arguments: vec!["first".into(), "second".into()],
        environment: Some(vec![("B".into(), "two".into()), ("A".into(), "one".into())]),
        ..Default::default()
    }
}
fn hash(key: &ApplicationRuntimeKey) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hash);
    hash.finish()
}

#[test]
fn normalized_keys_sort_environment_without_rewriting_authored_settings() {
    let original = definition();
    let saved = serde_json::to_value(&original).unwrap();
    let first = ApplicationRuntimeKey::new(&original).unwrap();
    let mut reordered = original.clone();
    reordered.environment.as_mut().unwrap().reverse();
    let second = ApplicationRuntimeKey::new(&reordered).unwrap();
    assert_eq!(first, second);
    assert_eq!(hash(&first), hash(&second));
    assert_eq!(serde_json::to_value(&original).unwrap(), saved);
    assert_eq!(first.environment.as_ref().unwrap()[0].0, "A");
}

#[test]
fn keys_keep_every_launch_and_control_setting_and_environment_mode_distinct() {
    let original = definition();
    let first = ApplicationRuntimeKey::new(&original).unwrap();
    let mut variants = Vec::new();
    let mut changed = original.clone();
    changed.arguments.reverse();
    variants.push(changed);
    let mut changed = original.clone();
    changed.environment = None;
    variants.push(changed);
    let mut changed = original.clone();
    changed.environment = Some(vec![]);
    variants.push(changed);
    let mut changed = original.clone();
    changed.environment.as_mut().unwrap()[0].1.push('!');
    variants.push(changed);
    let mut changed = original.clone();
    changed.window = ApplicationWindow::Hidden;
    variants.push(changed);
    let mut changed = original.clone();
    changed.readiness = ApplicationReadiness::Spout {
        sender: "sender".into(),
        first_frame: true,
        timeout_ms: 100,
    };
    variants.push(changed);
    let mut changed = original.clone();
    changed.stop.graceful_ms += 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.stop.retirement_ms += 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.restart.enabled = false;
    variants.push(changed);
    let mut changed = original.clone();
    changed.restart.initial_ms += 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.restart.maximum_ms += 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.directory = Some(
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .into(),
    );
    variants.push(changed);
    for variant in variants {
        assert_ne!(ApplicationRuntimeKey::new(&variant).unwrap(), first, "{variant:?}");
    }
    let inherited = ApplicationRuntimeKey::new(&ApplicationDefinition {
        environment: None,
        ..original.clone()
    })
    .unwrap();
    let empty = ApplicationRuntimeKey::new(&ApplicationDefinition {
        environment: Some(vec![]),
        ..original
    })
    .unwrap();
    assert_ne!(inherited, empty);
}

#[test]
fn malformed_configuration_cannot_construct_a_runtime_key() {
    for invalid in [
        ApplicationDefinition {
            executable: Some("relative.exe".into()),
            ..definition()
        },
        ApplicationDefinition {
            arguments: vec!["bad\0value".into()],
            ..definition()
        },
        ApplicationDefinition {
            environment: Some(vec![("NAME".into(), "a".into()), ("NAME".into(), "b".into())]),
            ..definition()
        },
        ApplicationDefinition {
            restart: ApplicationRestart {
                initial_ms: 0,
                ..Default::default()
            },
            ..definition()
        },
    ] {
        assert!(ApplicationRuntimeKey::new(&invalid).is_err());
    }
}

#[cfg(windows)]
#[test]
fn windows_absolute_spelling_is_normalized_without_resolving_files_or_verbatim_paths() {
    let original = ApplicationDefinition {
        executable: Some(r"c:\does-not-exist\sub\..\App.exe".into()),
        directory: Some(r"c:\does-not-exist\.".into()),
        ..definition()
    };
    let normalized = ApplicationRuntimeKey::new(&original).unwrap();
    let expected = ApplicationDefinition {
        executable: Some(r"C:\does-not-exist\App.exe".into()),
        directory: Some(r"C:\does-not-exist".into()),
        ..definition()
    };
    assert_eq!(normalized, ApplicationRuntimeKey::new(&expected).unwrap());
    let verbatim = ApplicationDefinition {
        executable: Some(r"\\?\C:\does-not-exist\sub\..\App.exe".into()),
        ..definition()
    };
    assert_eq!(
        ApplicationRuntimeKey::new(&verbatim).unwrap().executable,
        verbatim.executable
    );
    let different_case = ApplicationDefinition {
        executable: Some(r"C:\does-not-exist\app.exe".into()),
        ..expected
    };
    assert_ne!(
        normalized,
        ApplicationRuntimeKey::new(&different_case).unwrap(),
        "directory case sensitivity must not be guessed"
    );
}
