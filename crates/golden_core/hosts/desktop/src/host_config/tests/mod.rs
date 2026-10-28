use super::*;

#[test]
fn default_host_preserves_the_default_ui_listener() {
    let config = DesktopHostConfig::default();
    assert_eq!(config.ui_server.bind_addr, "127.0.0.1:7010");
    assert!(config.dev_server.is_none());
}

#[test]
fn application_listener_defaults_survive_absent_or_empty_operator_overrides() {
    let mut config = UiServerConfig {
        bind_addr: "127.0.0.1:7011".into(),
        allowed_hosts: vec!["mapgyver.local:7011".into()],
        ..Default::default()
    };
    apply_listener_overrides(&mut config, None, None, false);
    apply_listener_overrides(&mut config, Some("  "), None, false);
    assert_eq!(config.bind_addr, "127.0.0.1:7011");
    assert_eq!(config.allowed_hosts, ["mapgyver.local:7011"]);
}

#[test]
fn operator_overrides_follow_app_defaults_and_no_remote_runs_last() {
    let mut config = UiServerConfig {
        bind_addr: "127.0.0.1:7011".into(),
        allowed_hosts: vec!["app.local:7011".into()],
        ..Default::default()
    };
    apply_listener_overrides(
        &mut config,
        Some("0.0.0.0:7020"),
        Some(" operator.local:7020, localhost:7020 "),
        true,
    );
    assert_eq!(config.bind_addr, "127.0.0.1:7020");
    assert_eq!(config.allowed_hosts, ["operator.local:7020", "localhost:7020"]);

    apply_listener_overrides(&mut config, Some("[::]:7021"), None, true);
    assert_eq!(config.bind_addr, "[::1]:7021");
}
