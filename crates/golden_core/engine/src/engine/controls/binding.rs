//! Binding components use one winning author and one traversal, never ticking echoes.
use super::*;
use std::collections::VecDeque;

impl<T: Node> Engine<T> {
    /// The live target of a bidirectional binding, including any component projection.
    /// Missing targets remain authored references and return `None` until resolved.
    pub fn parameter_binding_target(&self, param: NodeId) -> Option<(NodeId, Option<ParamValueProjection>)> {
        let state = self.nodes.get(param)?.engine_param_control_state()?;
        if state.mode != ParameterControlMode::Binding {
            return None;
        }
        let config = self.read_reference_control_config(param, "binding", &mut Vec::new())?;
        let target = self.resolve_reference_target_node(&config.target)?;
        self.control_param_snapshot(target)?;
        Some((target, config.projection))
    }

    /// Resolved connected members, sorted by runtime identity. No topology is synthesized
    /// for missing UUIDs; restoring that identity reconnects its existing authored edges.
    pub fn parameter_binding_group(&self, param: NodeId) -> Vec<NodeId> {
        self.binding_component(vec![param])
    }

    pub(super) fn binding_component(&self, seeds: Vec<NodeId>) -> Vec<NodeId> {
        // During edit preparation/load the index can be stale. The read-only fallback is
        // used only then; ordinary value edits walk the indexed component, not the graph.
        let mut incoming = HashMap::<NodeId, Vec<NodeId>>::new();
        if self.control_index_dirty {
            for (id, _) in self.nodes.iter() {
                if let Some((target, _)) = self.parameter_binding_target(id) {
                    incoming.entry(target).or_default().push(id);
                }
            }
        }
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from(seeds);
        while let Some(param) = queue.pop_front() {
            if !seen.insert(param) {
                continue;
            }
            if let Some((target, _)) = self.parameter_binding_target(param) {
                queue.push_back(target);
            }
            let dependents = if self.control_index_dirty {
                incoming.get(&param)
            } else {
                self.control_source_dependents.get(&param)
            };
            for &dependent in dependents.into_iter().flatten() {
                if self
                    .parameter_binding_target(dependent)
                    .is_some_and(|(target, _)| target == param)
                {
                    queue.push_back(dependent);
                }
            }
        }
        let mut members: Vec<_> = seen.into_iter().collect();
        members.sort_by_key(|node| node.0);
        members
    }

    /// Queues an acyclic, same-value binding group using ordinary control/reference
    /// Parameters. The caller owns the surrounding edit transaction, as for other
    /// queued edits. Complete existing components must be supplied. External control
    /// modes and projected/dangling bindings are rejected before anything is queued.
    pub fn queue_parameter_binding_group(&mut self, members: &[NodeId], anchor: NodeId) -> Result<(), String> {
        let unique: HashSet<_> = members.iter().copied().collect();
        if members.len() < 2 || unique.len() != members.len() || !unique.contains(&anchor) {
            return Err("a binding group needs distinct members and an included anchor".into());
        }
        let value = self
            .control_param_snapshot(anchor)
            .ok_or("binding anchor is not a Parameter")?
            .value;
        let component = self.binding_component(members.to_vec());
        if component.iter().any(|node| !unique.contains(node)) {
            return Err("include every existing binding-group member".into());
        }
        for &param in members {
            let snapshot = self
                .control_param_snapshot(param)
                .ok_or("binding member is not a Parameter")?;
            if !matches!(
                snapshot.control.mode,
                ParameterControlMode::Manual | ParameterControlMode::Binding
            ) {
                return Err("binding group conflicts with an existing parameter control".into());
            }
            if snapshot.control.mode == ParameterControlMode::Binding
                && !self
                    .parameter_binding_target(param)
                    .is_some_and(|(_, projection)| projection.is_none())
            {
                return Err("binding group cannot replace projected or unresolved bindings".into());
            }
            if std::mem::discriminant(&snapshot.value) != std::mem::discriminant(&value)
                || self
                    .prepare_set_param_value(0, param, value.clone())
                    .map_err(|error| error.to_string())?
                    != value
                || !available_control_modes_for_parameter(&snapshot.value, snapshot.control_modes_enabled)
                    .contains(&ParameterControlMode::Binding)
            {
                return Err("binding group requires compatible values and constraints".into());
            }
        }
        let uuid = self.nodes.get(anchor).expect("validated anchor").node_data().meta.uuid;
        let target = ParamValue::Reference(NodeReference::new(uuid));
        for &param in members {
            if param == anchor {
                self.edits.push(Edit::SetParamControlState {
                    node: param,
                    state: ParameterControlState::new(ParameterControlMode::Manual, ParameterControlSpec::Manual),
                });
                continue;
            }
            if let Some(reference) = self.find_direct_child_by_decl_id(param, PARAMETER_CONTROL_REFERENCE_DECL_ID) {
                self.edits.push(Edit::SetParam {
                    node: reference,
                    value: target.clone(),
                    behaviour: ParameterEventBehaviour::Append,
                });
            } else {
                let mut reference = Self::make_control_reference_parameter();
                reference.value = target.clone();
                self.edits.push(Edit::AddNode {
                    node: Box::new(reference),
                    parent: param,
                    prev_sibling: None,
                });
            }
            self.edits.push(Edit::SetParamControlState {
                node: param,
                state: ParameterControlState::new(ParameterControlMode::Binding, ParameterControlSpec::Binding),
            });
        }
        self.edits.push(Edit::SetParamsAtomic {
            values: members.iter().map(|&param| (param, value.clone())).collect(),
            behaviour: ParameterEventBehaviour::Append,
        });
        Ok(())
    }

    pub(super) fn evaluate_binding_groups(
        &self,
        pairs: Vec<(NodeId, NodeId)>,
        targets: &HashMap<NodeId, BindingTargetConfig>,
        snapshots: &HashMap<NodeId, ParameterSnapshot>,
        diagnostics: &mut HashMap<NodeId, Vec<ParameterControlDiagnostic>>,
    ) -> Vec<(NodeId, ParamValue)> {
        let mut neighbors = HashMap::<NodeId, Vec<NodeId>>::new();
        for (a, b) in pairs {
            neighbors.entry(a).or_default().push(b);
            neighbors.entry(b).or_default().push(a);
        }
        let mut remaining: Vec<_> = neighbors.keys().copied().collect();
        remaining.sort_by_key(|node| node.0);
        let mut visited = HashSet::new();
        let mut writes = Vec::new();
        for seed in remaining {
            if visited.contains(&seed) {
                continue;
            }
            let mut members = vec![seed];
            visited.insert(seed);
            let mut offset = 0;
            while offset < members.len() {
                for &next in &neighbors[&members[offset]] {
                    if visited.insert(next) {
                        members.push(next);
                    }
                }
                offset += 1;
            }
            let source = *members
                .iter()
                .max_by_key(|node| {
                    (
                        self.param_last_change_counter.get(node).copied().unwrap_or(0),
                        std::cmp::Reverse(node.0),
                    )
                })
                .expect("nonempty component");
            let mut propagated = HashSet::from([source]);
            let mut queue = VecDeque::from([(source, snapshots[&source].value.clone())]);
            while let Some((source, value)) = queue.pop_front() {
                for &target in &neighbors[&source] {
                    if propagated.contains(&target) {
                        continue;
                    }
                    let mut errors = Vec::new();
                    if let Some(converted) = self.convert_binding_for_target(
                        source,
                        &value,
                        target,
                        &snapshots[&target],
                        targets,
                        &mut errors,
                    ) {
                        propagated.insert(target);
                        if converted != snapshots[&target].value {
                            writes.push((target, converted.clone()));
                        }
                        queue.push_back((target, converted));
                    }
                    diagnostics.entry(target).or_default().extend(errors);
                }
            }
        }
        writes.sort_by_key(|(node, _)| node.0);
        writes
    }
}
