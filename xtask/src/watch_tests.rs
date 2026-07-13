use crate::watch::is_parent_cargo_package_variable;
use std::ffi::OsStr;

#[test]
fn nested_cargo_drops_parent_package_context_but_keeps_user_configuration() {
    for name in [
        "CARGO_BIN_NAME",
        "CARGO_CRATE_NAME",
        "CARGO_MANIFEST_DIR",
        "CARGO_MANIFEST_PATH",
        "CARGO_PRIMARY_PACKAGE",
        "CARGO_TARGET_TMPDIR",
        "CARGO_BIN_EXE_Chataigne2",
        "CARGO_PKG_VERSION",
    ] {
        assert!(
            is_parent_cargo_package_variable(OsStr::new(name)),
            "{name} should not leak from xtask into nested Cargo"
        );
    }

    for name in [
        "CARGO",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_MAKEFLAGS",
        "CARGO_TARGET_DIR",
        "GC_UI_ASSUME_BUILT",
    ] {
        assert!(
            !is_parent_cargo_package_variable(OsStr::new(name)),
            "{name} is user or shared Cargo configuration and must be preserved"
        );
    }
}
