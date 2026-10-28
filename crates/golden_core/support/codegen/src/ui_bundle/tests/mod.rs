use super::*;

struct Fixture {
    root: tempfile::TempDir,
    ui_root: PathBuf,
    out_dir: PathBuf,
    shared_roots: Vec<PathBuf>,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let ui_root = root.path().join("apps/example/ui");
        let out_dir = root.path().join("cargo-out");
        fs::create_dir_all(&ui_root).unwrap();
        fs::create_dir_all(&out_dir).unwrap();
        write(root.path().join("package.json"), "{}");
        write(root.path().join("package-lock.json"), "{}");
        Self {
            shared_roots: vec![root.path().join("packages/shared-ui")],
            root,
            ui_root,
            out_dir,
        }
    }

    fn config(&self) -> UiBundleConfig<'_> {
        UiBundleConfig {
            workspace_root: self.root.path(),
            ui_root: &self.ui_root,
            npm_workspace: "example-ui",
            out_dir: &self.out_dir,
            shared_input_roots: &self.shared_roots,
            dependency_cache_namespace: "example",
        }
    }
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn cache_namespace_cannot_escape_its_directory() {
    let fixture = Fixture::new();
    for namespace in ["", ".", "..", "../outside", "nested/name", r"nested\name", "/absolute"] {
        let config = UiBundleConfig {
            dependency_cache_namespace: namespace,
            ..fixture.config()
        };
        assert_eq!(BundlePaths::new(&config).err().unwrap().kind(), ErrorKind::InvalidInput);
    }
    let config = fixture.config();
    let paths = BundlePaths::new(&config).unwrap();
    assert_eq!(
        paths.npm_deps_stamp,
        fixture
            .root
            .path()
            .join("node_modules/.cache/example/package-lock.hash")
    );
}

#[test]
fn app_selection_and_environment_separate_bundle_cache_contexts() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let paths = BundlePaths::new(&config).unwrap();
    let toolchain = "node=v26\nnpm=11";
    let baseline = bundle_context(&paths, toolchain, &[]);
    assert_eq!(baseline, bundle_context(&paths, toolchain, &[]));

    let other_workspace = UiBundleConfig {
        npm_workspace: "other-ui",
        ..fixture.config()
    };
    assert_ne!(
        baseline,
        bundle_context(&BundlePaths::new(&other_workspace).unwrap(), toolchain, &[])
    );
    let other_root = fixture.root.path().join("other-root");
    let other_root_config = UiBundleConfig {
        workspace_root: &other_root,
        ..fixture.config()
    };
    assert_ne!(
        baseline,
        bundle_context(&BundlePaths::new(&other_root_config).unwrap(), toolchain, &[])
    );
    let other_ui = fixture.root.path().join("other-ui");
    let other_ui_config = UiBundleConfig {
        ui_root: &other_ui,
        ..fixture.config()
    };
    assert_ne!(
        baseline,
        bundle_context(&BundlePaths::new(&other_ui_config).unwrap(), toolchain, &[])
    );
    let other_shared = [fixture.root.path().join("packages/other-shared")];
    let other_shared_config = UiBundleConfig {
        shared_input_roots: &other_shared,
        ..fixture.config()
    };
    assert_ne!(
        baseline,
        bundle_context(&BundlePaths::new(&other_shared_config).unwrap(), toolchain, &[])
    );
    assert_ne!(baseline, bundle_context(&paths, "node=v27\nnpm=12", &[]));
    assert_ne!(
        baseline,
        bundle_context(&paths, toolchain, &[("VITE_MODE".into(), "custom".into())])
    );
}

#[test]
fn identical_relative_config_from_different_callers_has_distinct_cache_identity() {
    let fixture = Fixture::new();
    let shared_roots = [PathBuf::from("../../packages/shared-ui")];
    let config = UiBundleConfig {
        workspace_root: Path::new("../.."),
        ui_root: Path::new("ui"),
        npm_workspace: "example-ui",
        out_dir: Path::new("cargo-out"),
        shared_input_roots: &shared_roots,
        dependency_cache_namespace: "example",
    };
    let first = BundlePaths::new_at(&config, fixture.root.path().join("checkout-one/apps/example")).unwrap();
    let second = BundlePaths::new_at(&config, fixture.root.path().join("checkout-two/apps/example")).unwrap();
    assert_ne!(
        bundle_context(&first, "node=26\nnpm=11", &[]),
        bundle_context(&second, "node=26\nnpm=11", &[])
    );
}

#[test]
fn another_npm_workspace_cannot_reuse_the_first_bundle() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let paths = BundlePaths::new(&config).unwrap();
    let other_config = UiBundleConfig {
        npm_workspace: "other-ui",
        ..fixture.config()
    };
    let other_paths = BundlePaths::new(&other_config).unwrap();
    let rebuilds = std::cell::Cell::new(0);
    let build = |selected: &BundlePaths<'_>| {
        build_if_stale(
            &selected.ui_bundle_stamp,
            &selected.bundled_ui_dir,
            &[],
            &bundle_context(selected, "node=26\nnpm=11", &[]),
            false,
            || {
                rebuilds.set(rebuilds.get() + 1);
                write(
                    selected.bundled_ui_dir.join("index.html"),
                    selected.config.npm_workspace,
                );
                Ok(())
            },
        )
        .unwrap()
    };
    assert!(build(&paths));
    assert!(!build(&paths));
    assert!(build(&other_paths));
    assert!(!build(&other_paths));
    assert_eq!(rebuilds.get(), 2);
    assert_eq!(
        fs::read_to_string(paths.bundled_ui_dir.join("index.html")).unwrap(),
        "other-ui"
    );
}

#[test]
fn only_selected_shared_inputs_and_source_files_are_tracked() {
    let fixture = Fixture::new();
    let expected = [
        fixture.ui_root.join("vite.config.ts"),
        fixture.ui_root.join(".env.production"),
        fixture.ui_root.join("src/main.ts"),
        fixture.ui_root.join("static/icon.svg"),
        fixture.shared_roots[0].join("src/widget.svelte"),
        fixture.root.path().join("tools/bootstrap/toolchain.json"),
    ];
    for path in &expected {
        write(path, "source");
    }
    let excluded = [
        fixture.ui_root.join("build/generated.js"),
        fixture.shared_roots[0].join("node_modules/dependency.js"),
        fixture.shared_roots[0].join("docs/readme.md"),
        fixture.shared_roots[0].join("tests/widget.rs"),
        fixture.root.path().join("packages/unselected/src/widget.svelte"),
    ];
    for path in &excluded {
        write(path, "not an input");
    }
    let config = fixture.config();
    let paths = BundlePaths::new(&config).unwrap();
    let inputs = ui_input_files(&paths).unwrap();
    for path in &expected {
        assert!(inputs.contains(path), "missing {}", path.display());
    }
    for path in &excluded {
        assert!(!inputs.contains(path), "unexpected {}", path.display());
    }
    assert!(inputs.contains(&PathBuf::from("build.rs")));
    assert!(inputs.windows(2).all(|pair| pair[0] < pair[1]));

    let added = fixture.shared_roots[0].join("src/new-widget.svelte");
    write(&added, "new source");
    assert!(ui_input_files(&paths).unwrap().contains(&added));
    fs::remove_file(&added).unwrap();
    assert!(!ui_input_files(&paths).unwrap().contains(&added));
}

#[test]
fn assets_use_stable_paths_mime_types_and_escaped_rust_literals() {
    let fixture = Fixture::new();
    let dist = fixture.ui_root.join("build");
    write(dist.join("index.html"), "<html></html>");
    write(dist.join("assets/a.JS"), "export {};");
    write(dist.join("assets/font.woff2"), "font");
    write(dist.join("assets/unknown.bin"), "data");
    let output = fixture.out_dir.join("ui_assets.rs");
    generate_ui_assets_module(&dist, &output).unwrap();
    let first = fs::read_to_string(&output).unwrap();
    generate_ui_assets_module(&dist, &output).unwrap();
    assert_eq!(first, fs::read_to_string(&output).unwrap());
    assert!(first.contains("pub static APP_UI_ASSETS: &[golden_core::app::UiAsset]"));
    assert!(first.contains("path: \"/assets/a.JS\""));
    assert!(first.contains("content_type: \"text/javascript; charset=utf-8\""));
    assert!(first.contains("content_type: \"font/woff2\""));
    assert!(first.contains("content_type: \"application/octet-stream\""));
    assert!(first.find("path: \"/assets/a.JS\"").unwrap() < first.find("path: \"/index.html\"").unwrap());
    let absolute = dist.join("assets/a.JS").canonicalize().unwrap();
    assert!(first.contains(&format!("include_bytes!({:?})", absolute.to_str().unwrap())));
}

#[cfg(unix)]
#[test]
fn asset_paths_with_quotes_cannot_end_the_generated_literal() {
    let fixture = Fixture::new();
    let dist = fixture.ui_root.join("build");
    write(dist.join("index.html"), "<html></html>");
    let asset = dist.join("quote\"#file.js");
    write(&asset, "source");
    let output = fixture.out_dir.join("ui_assets.rs");
    generate_ui_assets_module(&dist, &output).unwrap();
    let source = fs::read_to_string(output).unwrap();
    assert!(source.contains("path: \"/quote\\\"#file.js\""));
    assert!(source.contains(&format!(
        "include_bytes!({:?})",
        asset.canonicalize().unwrap().to_str().unwrap()
    )));
}

#[test]
fn missing_or_nested_index_does_not_create_an_asset_module() {
    let fixture = Fixture::new();
    let dist = fixture.ui_root.join("build");
    let output = fixture.out_dir.join("ui_assets.rs");
    assert_eq!(
        generate_ui_assets_module(&dist, &output).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    write(dist.join("nested/index.html"), "nested only");
    assert_eq!(
        generate_ui_assets_module(&dist, &output).unwrap_err().kind(),
        ErrorKind::InvalidData
    );
    assert!(!output.exists());
}

#[test]
fn skip_takes_precedence_and_prebuilt_embeds_without_npm() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let paths = BundlePaths::new(&config).unwrap();
    prepare_assets(&paths, BuildMode::from_flags(true, true), true).unwrap();
    assert!(
        fs::read_to_string(&paths.ui_assets_rs)
            .unwrap()
            .contains("UiAsset] = &[];")
    );
    assert!(!paths.npm_deps_stamp.exists());
    assert!(!paths.bundled_ui_dir.exists());

    write(fixture.ui_root.join("build/index.html"), "prebuilt");
    prepare_assets(&paths, BuildMode::from_flags(false, true), true).unwrap();
    assert!(
        fs::read_to_string(&paths.ui_assets_rs)
            .unwrap()
            .contains("path: \"/index.html\"")
    );
    assert!(!paths.npm_deps_stamp.exists());
    assert!(!paths.ui_bundle_stamp.exists());
}
