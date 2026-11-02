//! Pure coupled-parameter proposal preparation and exact canonical history restoration.
use super::history::{HistoryStep, SetParamEffect};
use super::{Engine, EngineEditError};
use crate::contexts::UserContextValueType;
use crate::node::{Node, NodeId, ParameterValidationContext};
use crate::parameter::{ParamValue, Parameter, ParameterEventBehaviour};
use std::collections::HashMap;

pub(super) enum ParameterWriteEffects {
    Scalar(Option<SetParamEffect>),
    Atomic(Vec<SetParamEffect>),
}

impl ParameterWriteEffects {
    pub(super) fn values(&self) -> &[SetParamEffect] {
        match self {
            Self::Scalar(effect) => effect.as_slice(),
            Self::Atomic(effects) => effects,
        }
    }
    pub(super) fn history<T: Node>(self) -> Option<HistoryStep<T>> {
        match self {
            Self::Scalar(effect) => effect.map(Into::into),
            Self::Atomic(effects) => (!effects.is_empty()).then(|| effects.into()),
        }
    }
    pub(super) fn set_policy(&mut self, behaviour: ParameterEventBehaviour, tick: u64) {
        let effects = match self {
            Self::Scalar(effect) => effect.as_mut_slice(),
            Self::Atomic(effects) => effects.as_mut_slice(),
        };
        for effect in effects {
            effect.behaviour = behaviour;
            effect.tick = tick;
        }
    }
}

impl<T: Node> Engine<T> {
    pub(super) fn parameter_effect_dependencies(&mut self, effects: &[SetParamEffect]) -> (bool, bool) {
        let mut references = false;
        let mut contexts = false;
        for effect in effects {
            references |= matches!(effect.old_value, ParamValue::Reference(_))
                || matches!(effect.new_value, ParamValue::Reference(_));
            contexts |= UserContextValueType::from_param_value(&effect.old_value)
                != UserContextValueType::from_param_value(&effect.new_value)
                && self.node_within_user_context_scope(effect.node);
            contexts |= self.queue_user_context_multiplex_resize_for_count(effect.node);
        }
        (references, contexts)
    }
    pub(super) fn apply_parameter_edit(
        &mut self,
        edit_index: usize,
        node: NodeId,
        value: ParamValue,
        source: Option<NodeId>,
    ) -> Result<ParameterWriteEffects, EngineEditError> {
        let original = (node, self.prepare_set_param_leaf(edit_index, node, value)?);
        let companions = self.gather_param_companions(edit_index, std::slice::from_ref(&original), None)?;
        if companions.is_empty() {
            let (value, _) = self.normalize_param_owners(edit_index, node, original.1, None)?;
            self.validate_param_owners(edit_index, node, &value, None)?;
            return self
                .commit_prepared_param_value(edit_index, node, value, source)
                .map(ParameterWriteEffects::Scalar);
        }
        self.require_atomic_parameter(edit_index, node)?;
        let mut prepared = Vec::with_capacity(companions.len() + 1);
        prepared.push(original);
        prepared.extend(companions);
        self.commit_param_proposal(edit_index, prepared, ParameterEventBehaviour::Append, source)
            .map(ParameterWriteEffects::Atomic)
    }

    pub(crate) fn apply_set_params_atomic(
        &mut self,
        edit_index: usize,
        values: Vec<(NodeId, ParamValue)>,
        behaviour: ParameterEventBehaviour,
        source: Option<NodeId>,
    ) -> Result<Vec<SetParamEffect>, EngineEditError> {
        let mut proposed = HashMap::with_capacity(values.len());
        let mut prepared = Vec::with_capacity(values.len());
        for (node, value) in values {
            self.require_atomic_parameter(edit_index, node)?;
            let value = self.prepare_set_param_leaf(edit_index, node, value)?;
            if proposed.insert(node, value.clone()).is_some() {
                return Err(self.proposal_error(edit_index, node, "atomic parameter IDs must be distinct"));
            }
            prepared.push((node, value));
        }
        let companions = self.gather_param_companions(edit_index, &prepared, Some(&proposed))?;
        prepared.extend(companions);
        self.commit_param_proposal(edit_index, prepared, behaviour, source)
    }

    fn gather_param_companions(
        &self,
        edit_index: usize,
        originals: &[(NodeId, ParamValue)],
        proposed: Option<&HashMap<NodeId, ParamValue>>,
    ) -> Result<Vec<(NodeId, ParamValue)>, EngineEditError> {
        let mut companions = Vec::new();
        let mut derived = HashMap::new();
        let lookup = |id| self.nodes.get(id).map(|node| node as &dyn Node);
        let resolve = |uuid| self.node_id_by_uuid(uuid);
        for (node, value) in originals {
            let target = self.nodes.get(*node).expect("prepared parameter exists");
            let change = ParameterValidationContext::new(*node, value, &lookup, &resolve);
            let change = match proposed {
                Some(values) => change.with_proposed(values),
                None => change,
            };
            let mut parent = target.node_data().parent;
            while let Some(id) = parent {
                let Some(owner) = self.nodes.get(id) else { break };
                let values = owner
                    .parameter_companion_changes(&change)
                    .map_err(|message| self.proposal_error(edit_index, *node, &message))?;
                for (id, value) in values {
                    self.require_atomic_parameter(edit_index, id)?;
                    let value = self.prepare_set_param_leaf(edit_index, id, value)?;
                    let original = change.proposed_value(id);
                    if let Some(existing) = original.or_else(|| derived.get(&id)) {
                        if *existing != value {
                            return Err(self.proposal_error(
                                edit_index,
                                id,
                                "companion parameter proposal conflicts with another proposed value",
                            ));
                        }
                    } else {
                        derived.insert(id, value.clone());
                        companions.push((id, value));
                    }
                }
                parent = owner.node_data().parent;
            }
        }
        Ok(companions)
    }

    fn commit_param_proposal(
        &mut self,
        edit_index: usize,
        prepared: Vec<(NodeId, ParamValue)>,
        behaviour: ParameterEventBehaviour,
        source: Option<NodeId>,
    ) -> Result<Vec<SetParamEffect>, EngineEditError> {
        let mut proposed = prepared.iter().cloned().collect::<HashMap<_, _>>();
        let mut normalized = Vec::with_capacity(prepared.len());
        let mut replacements = Vec::new();
        for (node, value) in prepared {
            let (value, changed) = self.normalize_param_owners(edit_index, node, value, Some(&proposed))?;
            if changed {
                replacements.push((node, value.clone()));
            }
            normalized.push((node, value));
        }
        for (node, value) in replacements {
            proposed.insert(node, value);
        }
        for (node, value) in &normalized {
            self.validate_param_owners(edit_index, *node, value, Some(&proposed))?;
        }
        let mut effects = Vec::with_capacity(normalized.len());
        for (node, value) in normalized {
            if let Some(mut effect) = self.commit_prepared_param_value(edit_index, node, value, source)? {
                effect.behaviour = behaviour;
                effects.push(effect);
            }
        }
        Ok(effects)
    }

    fn require_atomic_parameter(&self, edit_index: usize, node: NodeId) -> Result<(), EngineEditError> {
        let target = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation: "SetParamsAtomic",
            node,
        })?;
        if !target.as_any().is::<Parameter>() {
            return Err(EngineEditError::ParamEditTargetMismatch {
                edit_index,
                node,
                node_type: target.get_type().into(),
            });
        }
        Ok(())
    }

    fn proposal_error(&self, edit_index: usize, node: NodeId, message: &str) -> EngineEditError {
        EngineEditError::ParamConstraintViolation {
            edit_index,
            node,
            node_type: self
                .nodes
                .get(node)
                .map(|node| node.get_type())
                .unwrap_or("missing")
                .into(),
            message: message.into(),
        }
    }

    fn prepare_history_parameter(&self, node: NodeId, value: &ParamValue) -> Result<(), EngineEditError> {
        let target = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound {
            edit_index: 0,
            operation: "RestoreParameterHistory",
            node,
        })?;
        let current = target
            .engine_param_snapshot()
            .ok_or_else(|| self.proposal_error(0, node, "history target is not a parameter"))?;
        if !value.has_only_finite_numbers() || std::mem::discriminant(&current.value) != std::mem::discriminant(value) {
            return Err(self.proposal_error(
                0,
                node,
                "history parameter value must retain its canonical type and finite data",
            ));
        }
        let accepted = target
            .engine_prepare_param_value(value.clone())
            .map_err(|message| self.proposal_error(0, node, &message))?;
        if accepted != *value {
            return Err(self.proposal_error(
                0,
                node,
                "history parameter value violates its current intrinsic constraints",
            ));
        }
        Ok(())
    }

    pub(crate) fn restore_parameter_history(&mut self, node: NodeId, value: ParamValue) -> Result<(), EngineEditError> {
        self.prepare_history_parameter(node, &value)?;
        self.validate_param_owners(0, node, &value, None)?;
        self.commit_prepared_param_value(0, node, value, None)?;
        Ok(())
    }

    pub(crate) fn restore_atomic_parameter_history(
        &mut self,
        values: Vec<(NodeId, ParamValue)>,
    ) -> Result<(), EngineEditError> {
        let proposed = values.iter().cloned().collect::<HashMap<_, _>>();
        if proposed.len() != values.len() {
            return Err(self.proposal_error(0, values[0].0, "history parameter IDs must be distinct"));
        }
        for (node, value) in &values {
            self.require_atomic_parameter(0, *node)?;
            self.prepare_history_parameter(*node, value)?;
            self.validate_param_owners(0, *node, value, Some(&proposed))?;
        }
        // Captured canonical values are restored exactly. Current catalog-derived
        // normalization/companions must never rewrite an earlier authored transaction.
        for (node, value) in values {
            self.commit_prepared_param_value(0, node, value, None)?;
        }
        Ok(())
    }
}
