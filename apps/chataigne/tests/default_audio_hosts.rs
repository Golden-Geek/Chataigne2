#[cfg(all(target_os = "windows", feature = "asio"))]
#[test]
fn default_windows_build_compiles_asio() {
    assert!(
        golden_audio::compiled_cpal_backends()
            .into_iter()
            .any(|backend| backend.id().as_str() == "asio"),
        "ordinary Chataigne Windows builds must include the ASIO host"
    );
}
