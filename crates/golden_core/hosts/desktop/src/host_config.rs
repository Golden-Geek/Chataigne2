use golden_transport_server::UiServerConfig;

/// App-provided defaults for the shared desktop and headless host runtime.
#[derive(Clone, Default)]
pub struct DesktopHostConfig {
    /// UI listener, bundled assets, and transport policy for this application.
    ///
    /// The host derives Preferences persistence from the app lifecycle. Operator
    /// environment overrides and launch flags are applied after these defaults.
    pub ui_server: UiServerConfig,
    /// Optional frontend development process owned by the host.
    pub dev_server: Option<FrontendDevServerConfig>,
}

/// App-provided configuration for launching a frontend dev server.
#[derive(Debug, Clone, Copy)]
pub struct FrontendDevServerConfig {
    /// Absolute working directory containing the frontend package.json.
    pub working_dir: &'static str,
    /// NPM script to execute for the frontend dev server.
    pub npm_script: &'static str,
    /// URL where the dev server should become reachable.
    pub url: &'static str,
}

pub(super) fn apply_listener_overrides(
    config: &mut UiServerConfig,
    bind_override: Option<&str>,
    allowed_hosts_override: Option<&str>,
    no_remote: bool,
) {
    if let Some(bind_addr) = bind_override.filter(|value| !value.trim().is_empty()) {
        config.bind_addr = bind_addr.to_owned();
    }
    if let Some(authorities) = allowed_hosts_override {
        config.allowed_hosts = authorities
            .split(',')
            .map(|authority| authority.trim().to_owned())
            .collect();
    }
    if no_remote {
        config.bind_addr = super::desktop::force_loopback_bind_addr(&config.bind_addr);
    }
}

#[cfg(test)]
mod tests;
