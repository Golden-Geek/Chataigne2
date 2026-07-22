use serde_json::{Value, json};
use std::io::{self, Write};

use crate::readiness::UiReadiness;

fn emit(value: Value) {
    println!("{value}");
    let _ = io::stdout().flush();
}

pub fn status(component: &str, state: &str, detail: impl Into<String>) {
    emit(json!({
        "event": "watch.status",
        "version": 1,
        "component": component,
        "state": state,
        "detail": detail.into(),
    }));
}

pub fn ready(frontend_url: &str, backend_url: &str, frontend_port: u16, backend_port: u16, readiness: &UiReadiness) {
    emit(json!({
        "event": "watch.ready",
        "version": 2,
        "frontend": {
            "state": "ready",
            "url": frontend_url,
            "port": frontend_port,
            "probe": "http_get_root",
        },
        "backend": {
            "state": "ready",
            "url": backend_url,
            "port": backend_port,
            "probe": "http_get_ui_readiness",
        },
        "engine": {
            "state": "ready",
            "read_model_revision": readiness.read_model_revision,
            "probe": "ui_readiness_revision",
        },
        "session": {
            "state": "ready",
            "active_websocket_clients": readiness.active_websocket_clients,
            "active_subscribed_websocket_clients": readiness.active_subscribed_websocket_clients,
            "probe": "active_subscribed_websocket_session",
        },
        "ports": {
            "frontend": frontend_port,
            "backend": backend_port,
        },
    }));
}

pub fn error(message: &str) {
    emit(json!({
        "event": "watch.error",
        "version": 1,
        "message": message,
    }));
}
