use super::*;

golden_engine::define_node_enum!(
    enum DiscoveryTestNode {}
);

impl ProjectLifecycle for DiscoveryTestNode {
    fn application_id() -> &'static str {
        "fixture-app"
    }

    fn application_display_name() -> &'static str {
        "Fixture Application"
    }

    fn application_version() -> &'static str {
        "2.3.4"
    }

    fn app_data_directory_name() -> &'static str {
        "FixtureData"
    }
}

#[test]
fn discovery_uses_product_identity_and_relative_transport_endpoints() {
    assert_eq!(
        serde_json::to_value(ui_discovery_document::<DiscoveryTestNode>()).unwrap(),
        json!({
            "version": 1,
            "service": "golden-ui",
            "health_path": "/api/ui/health",
            "websocket_path": "/api/ui/ws",
            "relative_endpoints": true,
            "application": {
                "id": "fixture-app",
                "display_name": "Fixture Application",
                "version": "2.3.4",
                "data_directory_name": "FixtureData",
            },
            "protocol_version": UI_PROTOCOL_VERSION,
        })
    );
}
