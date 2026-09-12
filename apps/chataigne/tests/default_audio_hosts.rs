#[cfg(feature = "jack")]
#[test]
fn ordinary_desktop_build_compiles_the_platform_audio_hosts() {
    let ids: Vec<_> = golden_audio::compiled_cpal_backend_catalog()
        .into_iter()
        .map(|backend| backend.id.to_string())
        .collect();

    #[cfg(target_os = "windows")]
    let native = "wasapi";
    #[cfg(target_os = "macos")]
    let native = "coreaudio";
    #[cfg(target_os = "linux")]
    let native = "alsa";

    assert!(ids.iter().any(|id| id == native), "native host missing: {ids:?}");
    assert!(ids.iter().any(|id| id == "jack"), "JACK host missing: {ids:?}");

    #[cfg(all(target_os = "windows", feature = "asio"))]
    assert!(ids.iter().any(|id| id == "asio"), "ASIO host missing: {ids:?}");

    #[cfg(not(target_os = "windows"))]
    assert!(!ids.iter().any(|id| id == "asio"), "ASIO must be Windows-only: {ids:?}");
}
