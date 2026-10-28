use crate::{AppTarget, ParseOutcome, WatchConfig};

fn watch(args: &[&str]) -> WatchConfig {
    match WatchConfig::parse(args.iter().copied()).unwrap() {
        ParseOutcome::Watch(config) => config,
        ParseOutcome::Help => panic!("expected watch configuration"),
    }
}

#[test]
fn default_app_and_ports_remain_chataigne() {
    let config = watch(&["watch"]);
    assert_eq!(config.app, AppTarget::Chataigne);
    assert_eq!((config.frontend_port, config.backend_port), (5173, 7010));
    assert_eq!(config.app.cargo_package(), "Chataigne2");
    assert_eq!(config.app.ui_directory(), "apps/chataigne/ui");
}

#[test]
fn mapgyver_selects_its_package_ui_and_distinct_default_ports() {
    let config = watch(&["watch", "--app", "mapgyver"]);
    assert_eq!(config.app, AppTarget::MapGyver);
    assert_eq!((config.frontend_port, config.backend_port), (5174, 7011));
    assert_eq!(config.app.cargo_package(), "MapGyver");
    assert_eq!(config.app.ui_directory(), "apps/mapgyver/ui");
}

#[test]
fn explicit_ports_win_independently_of_app_flag_order() {
    for args in [
        vec![
            "watch",
            "--app",
            "mapgyver",
            "--frontend-port",
            "4200",
            "--backend-port",
            "7100",
        ],
        vec![
            "watch",
            "--frontend-port",
            "4200",
            "--backend-port",
            "7100",
            "--app",
            "mapgyver",
        ],
    ] {
        let config = watch(&args);
        assert_eq!((config.frontend_port, config.backend_port), (4200, 7100));
    }
}

#[test]
fn invalid_apps_missing_values_and_colliding_ports_fail_before_launch() {
    for args in [
        vec!["watch", "--app", "other"],
        vec!["watch", "--app"],
        vec!["watch", "--app", "mapgyver", "--frontend-port", "7011"],
    ] {
        assert!(WatchConfig::parse(args).is_err());
    }
}
