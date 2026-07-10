use super::*;

#[test]
fn surfaces_require_explicit_execution_classes_and_unique_names() {
    let mut registry = ScriptSurfaceRegistry::default();
    registry
        .register(ModuleScriptSurface {
            module_type: "osc".into(),
            methods: vec![ScriptMember {
                name: "send".into(),
                execution: ExecutionClass::AsyncIo,
            }],
            callbacks: vec!["messageReceived".into()],
            template: "function send(address, value) {}".into(),
        })
        .unwrap();
    assert_eq!(registry.len(), 1);
    assert_eq!(
        registry.get("osc").unwrap().methods[0].execution,
        ExecutionClass::AsyncIo
    );
}
