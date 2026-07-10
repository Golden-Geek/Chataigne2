use super::desktop::frontend_origin;

#[test]
fn frontend_origin_preserves_the_actual_dev_server_authority() {
    assert_eq!(
        frontend_origin("http://127.0.0.1:5173/dashboard?mode=dev"),
        Some("http://127.0.0.1:5173".to_string())
    );
    assert_eq!(
        frontend_origin("https://studio.example.test/ui"),
        Some("https://studio.example.test".to_string())
    );
}

#[test]
fn frontend_origin_rejects_non_browser_schemes_and_missing_authorities() {
    assert_eq!(frontend_origin("tauri://localhost"), None);
    assert_eq!(frontend_origin("http:///missing-host"), None);
}
