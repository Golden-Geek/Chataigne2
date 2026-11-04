//! Explicit display assignment is one pure proposal, including displaced peers and saved hints.
use super::*;
use crate::screens::{ScreenManagerNode, protocol::ScreenId};
use std::collections::HashSet;

impl ScreenNode {
    pub(super) fn selection_changes(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Vec<(NodeId, ParamValue)>, String> {
        if !change.is_proposed(self.display.id()) && !change.is_proposed(self.target_kind.id()) {
            return Ok(Vec::new());
        }
        let Some(parent) = self.node_data().parent.and_then(|id| change.node(id)) else {
            return Ok(Vec::new());
        };
        let Some(manager) = parent.as_any().downcast_ref::<ScreenManagerNode>() else {
            return Ok(Vec::new());
        };
        let values = self.values(Some(change));
        let identity = values.display.as_str();
        let displays = manager.displays();
        let mut changes = Vec::new();
        if change.is_proposed(self.display.id())
            && let Some(display) = manager.known_display(identity)
        {
            changes.extend(self.display_hint_values(display));
        }
        if values.kind.as_str() != "Fullscreen" || identity.is_empty() {
            return Ok(changes);
        }

        let mut peers = Vec::new();
        let mut next = parent.node_data().first_child;
        while let Some(id) = next {
            let node = change.node(id).ok_or("Screens child is missing")?;
            next = node.node_data().next_sibling;
            if id != self.id()
                && let Some(peer) = node.as_any().downcast_ref::<ScreenNode>()
            {
                peers.push(peer);
            }
        }
        let destination = physical_identity(self, Some(change), manager);
        let old_identity = self.authored_display_identity();
        let old_destination = physical_identity(self, None, manager);
        let old_available = displays.iter().any(|display| display.id == old_identity)
            || manager
                .resolved_display(ScreenId(self.node_data().meta.uuid), &self.display_hint())
                .is_some();
        let mut can_swap = self.target_kind.get_ref().as_str() == "Fullscreen"
            && !old_identity.is_empty()
            && old_available
            && old_destination != destination
            && !peers.iter().any(|peer| {
                let values = peer.values(Some(change));
                values.kind.as_str() == "Fullscreen"
                    && physical_identity(peer, Some(change), manager) == old_destination
            });
        let mut senders = peers
            .iter()
            .chain(std::iter::once(&self))
            .filter_map(|screen| {
                let values = screen.values(Some(change));
                (values.kind.as_str() == "Shared Texture").then_some(values.sender)
            })
            .collect::<HashSet<_>>();
        for peer in peers {
            let proposed = peer.values(Some(change));
            if proposed.kind.as_str() != "Fullscreen"
                || proposed.display.as_str().is_empty()
                || physical_identity(peer, Some(change), manager) != destination
            {
                continue;
            }
            if can_swap {
                changes.push((peer.display.id(), ParamValue::Enum(old_identity.clone())));
                changes.extend(peer.copy_display_hint_values(self));
                can_swap = false;
            } else {
                let sender = crate::screens::creation::unique_sender(&senders);
                senders.insert(sender.clone());
                changes.push((peer.target_kind.id(), ParamValue::Enum("Shared Texture".into())));
                changes.push((peer.sender.id(), ParamValue::Str(sender)));
            }
        }
        Ok(changes)
    }

    pub(super) fn log_selection_change(&self, parameter: NodeId, old: &ParamValue) {
        let message = if parameter == self.display.id() {
            let ParamValue::Enum(previous) = old else { return };
            let current = self.authored_display_identity();
            (previous != &current).then(|| {
                let hint = self.display_hint();
                let display = if hint.name.is_empty() {
                    "selected display"
                } else {
                    &hint.name
                };
                format!("{}: display reassigned to {display}", self.node_data().meta.label)
            })
        } else if parameter == self.target_kind.id()
            && old == &ParamValue::Enum("Fullscreen".into())
            && self.target_kind.get_ref().as_str() == "Shared Texture"
        {
            Some(format!(
                "{}: fullscreen assignment replaced with Shared Texture ({})",
                self.node_data().meta.label,
                self.sender.get_ref()
            ))
        } else {
            None
        };
        if let Some(message) = message {
            golden_core::logger::log_message(
                golden_core::logger::LogLevel::Info,
                "mapgyver.screens".into(),
                Some(self.id()),
                message,
            );
        }
    }
}

fn physical_identity(
    screen: &ScreenNode,
    change: Option<&ParameterValidationContext<'_>>,
    manager: &ScreenManagerNode,
) -> String {
    let identity = read(&screen.display, change).as_str().to_owned();
    let hint = screen.display_hint();
    // An explicitly changed token selects the current native entry. Recovery only applies
    // while this Screen still carries the exact saved destination evidence it was matched from.
    if identity == hint.identity
        && let Some(display) = manager.resolved_display(ScreenId(screen.node_data().meta.uuid), &hint)
    {
        return display.id.clone();
    }
    identity
}
