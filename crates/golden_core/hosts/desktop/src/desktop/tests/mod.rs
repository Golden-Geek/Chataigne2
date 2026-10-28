use super::*;

const DEV_SERVER: FrontendDevServerConfig = FrontendDevServerConfig {
    working_dir: "unused",
    npm_script: "dev",
    url: "http://127.0.0.1:5174",
};

#[test]
fn an_external_frontend_keeps_the_configured_application_url() {
    let endpoint = UiEndpoint {
        connect_addr: "127.0.0.1:7011".into(),
    };
    for use_dev_server in [false, true] {
        assert_eq!(
            default_frontend_url(&endpoint, &[], use_dev_server, Some(DEV_SERVER)),
            DEV_SERVER.url
        );
    }
}

#[test]
fn no_frontend_admits_only_the_selected_application_origin() {
    let endpoint = UiEndpoint {
        connect_addr: "127.0.0.1:7011".into(),
    };
    let args = LaunchArgs {
        no_frontend: true,
        ..Default::default()
    };
    let frontend_url = default_frontend_url(&endpoint, &[], args.dev, Some(DEV_SERVER));
    let mut config = UiServerConfig::default();
    configure_frontend_origin(&mut config, &args, &frontend_url, false).unwrap();
    assert_eq!(config.allowed_origins, ["http://127.0.0.1:5174"]);
}

#[test]
fn bundled_assets_use_the_backend_unless_development_mode_is_requested() {
    let endpoint = UiEndpoint {
        connect_addr: "127.0.0.1:7011".into(),
    };
    let assets = [UiAsset {
        path: "/index.html",
        content_type: "text/html",
        bytes: b"frontend",
    }];
    assert_eq!(
        default_frontend_url(&endpoint, &assets, false, Some(DEV_SERVER)),
        "http://127.0.0.1:7011/"
    );
    assert_eq!(
        default_frontend_url(&endpoint, &assets, true, Some(DEV_SERVER)),
        DEV_SERVER.url
    );
}
