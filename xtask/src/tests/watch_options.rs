use crate::{ParseOutcome, WatchConfig};

fn watch(args: &[&str]) -> WatchConfig {
    match WatchConfig::parse(args.iter().copied()).unwrap() {
        ParseOutcome::Watch(config) => config,
        ParseOutcome::Help => panic!("expected watch configuration"),
    }
}

#[test]
fn default_ports_match_the_chataigne_hosts() {
    let config = watch(&["watch"]);
    assert_eq!((config.frontend_port, config.backend_port), (5173, 7010));
}

#[test]
fn explicit_ports_override_defaults() {
    let config = watch(&["watch", "--frontend-port", "4200", "--backend-port", "7100"]);
    assert_eq!((config.frontend_port, config.backend_port), (4200, 7100));
}

#[test]
fn missing_values_and_colliding_ports_fail_before_launch() {
    for args in [
        vec!["watch", "--frontend-port"],
        vec!["watch", "--backend-port", "5173"],
        vec!["watch", "--frontend-port", "7010"],
    ] {
        assert!(WatchConfig::parse(args).is_err());
    }
}
