//! Connection compatibility and whole-input/component replacement are app policy.

use super::*;
use chataigne_alchemist::{AlchemistGraphTransaction, SocketId, ValueComponent};

fn input_root(socket: &str) -> (&str, bool) {
    socket
        .rsplit_once('.')
        .filter(|(_, component)| ValueComponent::parse(component).is_some())
        .map_or((socket, false), |(root, _)| (root, true))
}

fn conflicts(existing: &str, requested: &str) -> bool {
    let (existing_root, existing_component) = input_root(existing);
    let (requested_root, requested_component) = input_root(requested);
    existing_root == requested_root && (!existing_component || !requested_component || existing == requested)
}

pub(super) fn connect(
    engine: &mut crate::app::AppEngine,
    request: FormulaConnectRequest,
) -> Result<FormulaConnectResult, String> {
    owner(engine, &request.formula, true)?;
    let snapshot = engine.process_tree_snapshot();
    for endpoint in [&request.from, &request.to] {
        if endpoint.socket.is_empty() || endpoint.socket.len() > 256 {
            return Err("Socket requires 1..256 bytes".into());
        }
        snapshot
            .node(endpoint.node)
            .filter(|node| {
                node.node_type == ANODE_NODE_TYPE
                    && node.parent == Some(request.formula.node)
                    && node.uuid == endpoint.uuid
            })
            .ok_or("Connection endpoint identity changed or belongs to another Formula")?;
    }
    let mut formula = formula_from_snapshot(&snapshot, request.formula.node)?;
    let domain = AlchemistGraphDomain::new(
        (*registry()).clone(),
        (*value_types()).clone(),
        Some(formula.properties.clone()),
    );
    let from = OutputSocketRef::new(
        ANodeId::from_uuid(request.from.uuid.0),
        SocketId::new(&request.from.socket),
    );
    let to = InputSocketRef::new(ANodeId::from_uuid(request.to.uuid.0), SocketId::new(&request.to.socket));
    let mut replaced = Vec::new();
    let mut existing = None;
    let mut conflict_ports = HashSet::new();
    for child in snapshot.child_ids(request.formula.node) {
        let Some(node) = snapshot
            .node(child)
            .filter(|node| node.node_type == CONNECTION_NODE_TYPE)
        else {
            continue;
        };
        if child_reference_uuid(&snapshot, child, "target_node") != Some(request.to.uuid) {
            continue;
        }
        let target_socket =
            child_string(&snapshot, child, "target_socket").ok_or("Connection target socket is missing")?;
        if !conflicts(&target_socket, &request.to.socket) {
            continue;
        }
        conflict_ports.insert(AlchemistGraphDomain::input_port_id(&SocketId::new(&target_socket)));
        if target_socket == request.to.socket
            && child_reference_uuid(&snapshot, child, "source_node") == Some(request.from.uuid)
            && child_string(&snapshot, child, "source_socket").as_deref() == Some(request.from.socket.as_str())
        {
            existing = Some(FormulaOwnerDto {
                node: child,
                uuid: node.uuid,
            });
        } else {
            replaced.push(FormulaOwnerDto {
                node: child,
                uuid: node.uuid,
            });
        }
    }
    let mut transaction = AlchemistGraphTransaction::for_document(&formula.graph);
    let target_node = AlchemistGraphDomain::node_id(to.node);
    for edge in formula.graph.edges() {
        if edge.to.node == target_node && conflict_ports.contains(&edge.to.port) {
            transaction.disconnect(edge.id);
        }
    }
    AlchemistGraphDomain::connect(&mut transaction, &formula.graph, from, to);
    transaction
        .commit(&mut formula.graph, &domain)
        .map_err(|error| error.to_string())?;
    let solved = solve_document_types(
        &formula.graph,
        &TypeSolveCtx {
            nodes: registry(),
            value_types: value_types(),
            properties: Some(&formula.properties),
        },
    );
    if let Some(error) = solved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(format!("Connection type validation failed: {}", error.message));
    }
    // No engine edit occurs until the entire replacement graph is admitted.
    for edge in &replaced {
        engine.edits.push(Edit::RemoveNode { node: edge.node });
    }
    let connection_uuid = if let Some(connection) = existing {
        connection.uuid
    } else {
        let node = AlchemistConnection::new();
        let uuid = node.node_data().meta.uuid;
        let mut tree = NodeTree::new(node).as_user_item();
        for (decl_id, value) in [
            (
                "source_node",
                ParamValue::Reference(NodeReference::new(request.from.uuid)),
            ),
            ("source_socket", ParamValue::Str(request.from.socket)),
            (
                "target_node",
                ParamValue::Reference(NodeReference::new(request.to.uuid)),
            ),
            ("target_socket", ParamValue::Str(request.to.socket)),
        ] {
            tree.push_child(NodeTree::new(parameter(decl_id, decl_id, value, false)));
        }
        engine.add_user_item_tree(tree, Some(request.formula.node));
        uuid
    };
    engine.apply_edits().map_err(|error| error.to_string())?;
    let connection = FormulaOwnerDto {
        node: engine
            .node_id_by_uuid(connection_uuid)
            .ok_or("Connection identity was retired")?,
        uuid: connection_uuid,
    };
    Ok(FormulaConnectResult { connection, replaced })
}
