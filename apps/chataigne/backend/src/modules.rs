use std::collections::BTreeMap;

use golden_io::{EndpointPolicy, IngressPolicy, RecoveryPolicy};
use golden_script::{ExecutionClass, ModuleScriptSurface, ScriptMember, ScriptSurfaceRegistry};
use smol_str::SmolStr;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModuleFamily {
    Protocol,
    Device,
    Generator,
    System,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDescriptor {
    pub type_id: SmolStr,
    pub family: ModuleFamily,
    pub endpoint: EndpointPolicy,
    pub commands: Vec<SmolStr>,
    pub script: ModuleScriptSurface,
}

pub struct ModuleCatalog {
    modules: BTreeMap<SmolStr, ModuleDescriptor>,
    scripts: ScriptSurfaceRegistry,
}

impl ModuleCatalog {
    pub fn get(&self, type_id: &str) -> Option<&ModuleDescriptor> {
        self.modules.get(type_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ModuleDescriptor> {
        self.modules.values()
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    pub fn scripts(&self) -> &ScriptSurfaceRegistry {
        &self.scripts
    }
}

pub fn chataigne_module_catalog() -> ModuleCatalog {
    let entries = [
        ("osc", ModuleFamily::Protocol, &["send"] as &[&str]),
        ("midi", ModuleFamily::Protocol, &["send_note", "send_control"]),
        ("artnet", ModuleFamily::Protocol, &["send_dmx"]),
        ("sacn", ModuleFamily::Protocol, &["send_dmx"]),
        ("serial", ModuleFamily::Protocol, &["write"]),
        ("mqtt", ModuleFamily::Protocol, &["publish", "subscribe"]),
        ("http", ModuleFamily::Protocol, &["request"]),
        ("tcp_client", ModuleFamily::Protocol, &["send"]),
        ("tcp_server", ModuleFamily::Protocol, &["send_to_client"]),
        ("udp", ModuleFamily::Protocol, &["send"]),
        ("websocket_client", ModuleFamily::Protocol, &["send"]),
        ("websocket_server", ModuleFamily::Protocol, &["broadcast"]),
        ("gamepad", ModuleFamily::Device, &["rumble"]),
        ("joycon", ModuleFamily::Device, &["rumble", "set_leds"]),
        ("keyboard", ModuleFamily::Device, &["press", "release"]),
        ("mouse", ModuleFamily::Device, &["move_to", "click"]),
        ("kinect2", ModuleFamily::Device, &["refresh"]),
        ("streamdeck", ModuleFamily::Device, &["set_key_image"]),
        ("ultraleap", ModuleFamily::Device, &["refresh"]),
        ("buttplug", ModuleFamily::Device, &["vibrate", "stop"]),
        ("signal", ModuleFamily::Generator, &["reset"]),
        ("metronome", ModuleFamily::Generator, &["tap", "reset"]),
        ("spatializer", ModuleFamily::Generator, &["set_source"]),
        ("app_control", ModuleFamily::System, &["launch", "focus", "quit"]),
        ("os", ModuleFamily::System, &["open", "execute"]),
        ("node", ModuleFamily::System, &["set_value", "trigger"]),
    ];
    let mut modules = BTreeMap::new();
    let mut scripts = ScriptSurfaceRegistry::default();
    for (type_id, family, commands) in entries {
        let methods = commands
            .iter()
            .map(|name| ScriptMember {
                name: (*name).into(),
                execution: ExecutionClass::AsyncIo,
            })
            .collect::<Vec<_>>();
        let script = ModuleScriptSurface {
            module_type: type_id.into(),
            methods,
            callbacks: vec!["connectionChanged".into(), "valueReceived".into()],
            template: format!(
                "// Chataigne {type_id} module\nfunction connectionChanged(connected) {{}}\nfunction valueReceived(value) {{}}"
            ),
        };
        scripts
            .register(script.clone())
            .expect("static module script surface is valid");
        let endpoint = EndpointPolicy {
            id: type_id.into(),
            queue_capacity: 1_024,
            ingress: IngressPolicy::LatestWins,
            recovery: RecoveryPolicy::default(),
        };
        endpoint.validate().expect("static endpoint policy is valid");
        modules.insert(
            type_id.into(),
            ModuleDescriptor {
                type_id: type_id.into(),
                family,
                endpoint,
                commands: commands.iter().map(|value| (*value).into()).collect(),
                script,
            },
        );
    }
    ModuleCatalog { modules, scripts }
}
