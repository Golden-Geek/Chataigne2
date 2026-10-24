use super::*;

const HTML: UiAsset = UiAsset {
    path: "/index.html",
    content_type: "text/html; charset=utf-8",
    bytes: b"<!doctype html><SCRIPT>\r\nwindow.ready = '&amp;';\r\n</SCRIPT><script src='/app.js'>ignored()</script>",
};

#[test]
fn policy_hashes_browser_normalized_inline_text_and_keeps_authority_exact() {
    let security = FrontendSecurity::prepare(&[HTML]).unwrap();
    let policy = security.policy(&HTML, "[::1]:7041").unwrap();
    let expected = STANDARD.encode(Sha256::digest(b"\nwindow.ready = '&amp;';\n"));
    assert!(policy.contains(&format!("'sha256-{expected}'")));
    assert!(!policy.contains(&STANDARD.encode(Sha256::digest(b"ignored()"))));
    let scripts = policy
        .split(';')
        .find(|part| part.trim_start().starts_with("script-src"))
        .unwrap();
    assert!(!scripts.contains("unsafe-inline"));
    assert!(!scripts.contains("unsafe-eval"));
    assert!(policy.contains("connect-src 'self' ws://[::1]:7041 ipc: http://ipc.localhost"));
    assert!(policy.contains("frame-ancestors 'none'"));
    assert!(policy.contains("object-src 'none'"));
}

#[test]
fn policy_is_prepared_for_each_html_asset_and_not_script_or_json_assets() {
    const OTHER: UiAsset = UiAsset {
        path: "/other.html",
        content_type: "text/html",
        bytes: b"<script>other()</script>",
    };
    const JSON: UiAsset = UiAsset {
        path: "/data.json",
        content_type: "application/json",
        bytes: b"{}",
    };
    let security = FrontendSecurity::prepare(&[HTML, OTHER, JSON]).unwrap();
    assert_ne!(
        security.policy(&HTML, "localhost:7010"),
        security.policy(&OTHER, "localhost:7010")
    );
    assert!(security.policy(&JSON, "localhost:7010").is_none());
    assert!(FrontendSecurity::default().policy(&HTML, "localhost:7010").is_none());
}

#[test]
fn duplicate_paths_invalid_utf8_and_excessive_inline_scripts_fail_startup() {
    assert!(FrontendSecurity::prepare(&[HTML, HTML]).is_err());
    const INVALID: UiAsset = UiAsset {
        path: "/bad.html",
        content_type: "text/html",
        bytes: &[255],
    };
    assert!(FrontendSecurity::prepare(&[INVALID]).is_err());
    let bytes = "<script>1</script>"
        .repeat(MAX_INLINE_SCRIPTS + 1)
        .into_bytes()
        .into_boxed_slice();
    let assets = Box::leak(Box::new([UiAsset {
        path: "/many.html",
        content_type: "text/html",
        bytes: Box::leak(bytes),
    }]));
    assert!(FrontendSecurity::prepare(assets).is_err());
}
