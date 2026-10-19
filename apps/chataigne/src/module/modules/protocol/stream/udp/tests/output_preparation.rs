use super::*;

#[test]
fn immediate_endpoint_edit_invalidates_the_cache_without_reinterpreting_an_accepted_send() {
    let old = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let new = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    old.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    new.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let (mut engine, module) = create_udp_module();
    let connection = child_by_decl(&engine, module, "connection");
    let outputs = child_by_decl(&engine, connection, "outputs");
    let output = engine.nodes.get(outputs).unwrap().node_data().first_child.unwrap();
    let port = child_by_decl(&engine, output, "remote_port");
    for (socket, text) in [(&old, "old"), (&new, "new")] {
        set_param(
            &mut engine,
            port,
            ParamValue::Int(i32::from(socket.local_addr().unwrap().port())),
        );
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: module,
            method: "sendText".into(),
            args: vec![ParamValue::Str(text.into())],
        });
        engine.apply_edits().unwrap();
    }
    let mut bytes = [0; 16];
    assert_eq!(old.recv(&mut bytes).unwrap(), 3);
    assert_eq!(&bytes[..3], b"old");
    assert_eq!(new.recv(&mut bytes).unwrap(), 3);
    assert_eq!(&bytes[..3], b"new");
    old.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    assert!(old.recv(&mut bytes).is_err());
}

#[test]
fn unavailable_transport_rejects_before_retaining_a_destination_plan() {
    let (mut engine, module) = create_udp_module();
    let crate::app::AppNode::UdpModule(node) = engine.nodes.get_mut(module).unwrap() else {
        panic!("UDP module");
    };
    node.transports.cancel_all();
    node.output_plan = None;
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: module,
        method: "sendText".into(),
        args: vec![ParamValue::Str("rejected".into())],
    });
    assert!(engine.apply_edits().is_err());
    assert!(
        udp_module(&engine, module).output_plan.is_none(),
        "unavailable modules must not occupy shared payload slots"
    );
}
