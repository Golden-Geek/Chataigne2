use super::super::{
    transport::{self, MqttCredentials, MqttSubscriptionConfig},
    MQTT_SUBSCRIPTION_NODE_TYPE,
};
use super::*;
use crate::app::module::common::mqtt::MqttQos;
use golden_core::parameter::ParamValue;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn build(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    cancelled: &AtomicBool,
) -> Result<MqttTransportConfig, Error> {
    if !snapshot.node(module).is_some_and(|node| node.enabled) {
        return Err(Error::invalid("MQTT module is absent or disabled"));
    }
    let connection = snapshot
        .find_child_by_decl_id(module, "connection")
        .ok_or_else(|| Error::invalid("MQTT connection is absent"))?;
    let remote_host = text(snapshot, connection, "remote_host", transport::MAX_HOST_BYTES)?
        .trim()
        .to_owned();
    if remote_host.is_empty() {
        return Err(Error::invalid("MQTT remote host cannot be empty"));
    }
    let client_id = text(snapshot, connection, "client_id", transport::MAX_CLIENT_BYTES)?.trim();
    let client_id = if client_id.is_empty() {
        format!("chataigne2-{}", module.0)
    } else {
        client_id.to_owned()
    };
    let remote_port = match value(snapshot, connection, "remote_port")? {
        ParamValue::Int(port) => {
            u16::try_from(*port).map_err(|_| Error::invalid("MQTT remote port must be between 0 and 65535"))?
        }
        _ => return Err(Error::invalid("MQTT remote port must be an integer")),
    };
    let clean_session = match value(snapshot, connection, "clean_session")? {
        ParamValue::Bool(value) => *value,
        _ => return Err(Error::invalid("MQTT Clean Session must be boolean")),
    };
    let keep_alive_secs = match value(snapshot, connection, "keep_alive_secs")? {
        ParamValue::Int(value) if (1..=65535).contains(value) => *value as u64,
        _ => return Err(Error::invalid("MQTT Keep Alive must be an integer between 1 and 65535")),
    };
    let credentials = snapshot
        .find_child_by_decl_id(connection, "credentials")
        .filter(|id| snapshot.node(*id).is_some_and(|node| node.enabled))
        .map(|id| {
            let username = text(snapshot, id, "username", transport::MAX_CREDENTIAL_BYTES)?
                .trim()
                .to_owned();
            if username.is_empty() {
                return Err(Error::invalid(
                    "MQTT username cannot be empty when Credentials is enabled",
                ));
            }
            Ok(MqttCredentials {
                username,
                password: text(snapshot, id, "password", transport::MAX_CREDENTIAL_BYTES)?.to_owned(),
            })
        })
        .transpose()?;
    let mut subscriptions = Vec::with_capacity(transport::MAX_SUBSCRIPTIONS);
    if let Some(root) = snapshot
        .find_child_by_decl_id(connection, "subscriptions")
        .filter(|id| snapshot.node(*id).is_some_and(|node| node.enabled))
    {
        let mut pending = Vec::new();
        extend(snapshot, root, 0, &mut pending)?;
        let mut visited = 0;
        while let Some(id) = pending.pop() {
            if cancelled.load(Ordering::Acquire) {
                return Err(Error::unavailable("MQTT configuration preparation cancelled"));
            }
            visited += 1;
            if visited > MAX_SCAN {
                return Err(Error::invalid("MQTT subscription structure exceeds 8192 nodes"));
            }
            let node = snapshot
                .node(id)
                .ok_or_else(|| Error::invalid("MQTT subscription node disappeared"))?;
            if node.node_type == MQTT_SUBSCRIPTION_NODE_TYPE {
                if !node.enabled {
                    continue;
                }
                if subscriptions.len() == transport::MAX_SUBSCRIPTIONS {
                    return Err(Error::invalid("MQTT configuration exceeds 128 subscriptions"));
                }
                let topic_filter = text(snapshot, id, "topic_filter", transport::MAX_FILTER_BYTES)?.trim();
                if !rumqttc::valid_filter(topic_filter) {
                    return Err(Error::invalid("Invalid MQTT subscription filter"));
                }
                let qos = MqttQos::from_variant(text(snapshot, id, "qos", 32)?)
                    .ok_or_else(|| Error::invalid("Invalid MQTT subscription QoS"))?;
                subscriptions.push(MqttSubscriptionConfig {
                    topic_filter: topic_filter.to_owned(),
                    qos,
                });
            } else if node.node_type == "folder" {
                extend(snapshot, id, visited, &mut pending)?;
            }
        }
    }
    Ok(MqttTransportConfig {
        remote_host,
        remote_port,
        client_id,
        credentials,
        clean_session,
        keep_alive_secs,
        subscriptions,
    })
}

fn value<'a>(snapshot: &'a ProcessTreeSnapshot, parent: NodeId, name: &str) -> Result<&'a ParamValue, Error> {
    snapshot
        .find_child_by_decl_id(parent, name)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .ok_or_else(|| Error::invalid(format!("MQTT configuration field {name} is absent")))
}
fn text<'a>(snapshot: &'a ProcessTreeSnapshot, parent: NodeId, name: &str, limit: usize) -> Result<&'a str, Error> {
    match value(snapshot, parent, name)? {
        ParamValue::Str(text) | ParamValue::Enum(text) if text.len() <= limit && !text.contains('\0') => Ok(text),
        _ => Err(Error::invalid(format!(
            "MQTT configuration field {name} must be text of at most {limit} bytes"
        ))),
    }
}
fn extend(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    visited: usize,
    pending: &mut Vec<NodeId>,
) -> Result<(), Error> {
    let children = snapshot.child_ids_slice(root);
    if visited + pending.len() + children.len() > MAX_SCAN {
        return Err(Error::invalid("MQTT subscription structure exceeds 8192 nodes"));
    }
    // Stack reversal preserves authored DFS order without per-folder child copies.
    pending.extend(children.iter().rev().copied());
    Ok(())
}
