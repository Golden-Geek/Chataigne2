use super::*;

impl From<&ProjectLoadRecoveryReport> for UiProjectLoadRecoveryDto {
    fn from(report: &ProjectLoadRecoveryReport) -> Self {
        Self {
            problems: report
                .problems
                .iter()
                .map(|problem| UiProjectLoadProblemDto {
                    stage: problem.stage.as_str().to_string(),
                    message: problem.message.clone(),
                })
                .collect(),
        }
    }
}

impl From<ProjectLoadRecoveryReport> for UiProjectLoadRecoveryDto {
    fn from(report: ProjectLoadRecoveryReport) -> Self {
        Self::from(&report)
    }
}

impl From<UserCreatableItem> for UiCreatableUserItemDto {
    fn from(item: UserCreatableItem) -> Self {
        Self {
            node_type: item.node_type,
            item_kind: item.item_kind,
            label: item.label,
            menu_path: item.menu_path,
            initial_params: item.initial_params.into_iter().map(Into::into).collect(),
            select_when_created: item.select_when_created,
            separator_before: item.separator_before,
            icon: item.icon,
        }
    }
}

impl From<UserCreatableItemInitialParam> for UiCreateUserItemInitialParam {
    fn from(initial_param: UserCreatableItemInitialParam) -> Self {
        Self {
            decl_id: initial_param.decl_id,
            value: initial_param.value,
        }
    }
}

impl From<&EngineNodeMetaPatch> for UiNodeMetaPatch {
    fn from(patch: &EngineNodeMetaPatch) -> Self {
        Self {
            label: patch.label.clone(),
            short_name: patch.short_name.clone(),
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            description: patch.description.clone(),
            user_permissions: patch.user_permissions.clone(),
            tags: patch.tags.clone(),
            presentation: patch.presentation.clone(),
        }
    }
}

impl From<EngineNodeMetaPatch> for UiNodeMetaPatch {
    fn from(patch: EngineNodeMetaPatch) -> Self {
        Self::from(&patch)
    }
}

impl From<UiNodeMetaPatch> for EngineNodeMetaPatch {
    fn from(patch: UiNodeMetaPatch) -> Self {
        Self {
            short_name: patch.short_name,
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            label: patch.label,
            description: patch.description,
            tags: patch.tags,
            user_permissions: patch.user_permissions,
            semantics: None,
            presentation: patch.presentation,
        }
    }
}

impl From<&EngineNodeMetaPatch> for NodeMetaPatch {
    fn from(patch: &EngineNodeMetaPatch) -> Self {
        Self {
            short_name: patch.short_name.clone(),
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            label: patch.label.clone(),
            description: patch.description.clone(),
            tags: patch.tags.clone(),
            user_permissions: patch.user_permissions.clone(),
            semantics: patch.semantics.as_ref().map(|semantics| SemanticsHint {
                intent: semantics.intent.clone(),
                unit: semantics.unit.clone(),
            }),
            presentation: patch.presentation.clone(),
        }
    }
}

impl From<NodeMetaPatch> for EngineNodeMetaPatch {
    fn from(patch: NodeMetaPatch) -> Self {
        Self {
            short_name: patch.short_name,
            enabled: patch.enabled,
            can_be_disabled: patch.can_be_disabled,
            label: patch.label,
            description: patch.description,
            tags: patch.tags,
            user_permissions: patch.user_permissions,
            semantics: patch.semantics.map(|semantics| crate::node::SemanticsHint {
                intent: semantics.intent,
                unit: semantics.unit,
            }),
            presentation: patch.presentation,
        }
    }
}

impl From<EngineNodeMetaPatch> for NodeMetaPatch {
    fn from(patch: EngineNodeMetaPatch) -> Self {
        Self::from(&patch)
    }
}

impl From<Event> for UiEventDto {
    fn from(event: Event) -> Self {
        let kind = match event.kind {
            EventKind::ParamChanged {
                param,
                old_value,
                new_value,
            } => UiEventKind::ParamChanged {
                param,
                old_value,
                new_value,
            },
            EventKind::ParamControlChanged {
                param,
                old_state,
                new_state,
            } => UiEventKind::ParamControlChanged {
                param,
                old_state: old_state.into(),
                new_state: new_state.into(),
            },
            EventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            } => UiEventKind::ParamConstraintsChanged {
                param,
                old_constraints,
                new_constraints,
            },
            EventKind::ChildAdded { parent, child, decl_id } => UiEventKind::ChildAdded {
                parent,
                child,
                decl_id,
                parent_children: None,
            },
            EventKind::ChildRemoved { parent, child } => UiEventKind::ChildRemoved { parent, child },
            EventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            } => UiEventKind::ChildReplaced {
                parent,
                old,
                new,
                decl_id,
            },
            EventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
            } => UiEventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
                old_parent_children: None,
                new_parent_children: None,
            },
            EventKind::ChildReordered { parent, child } => UiEventKind::ChildReordered {
                parent,
                child,
                parent_children: None,
            },
            EventKind::NodeCreated { node } => UiEventKind::NodeCreated { node, snapshot: None },
            EventKind::NodeDeleted { node } => UiEventKind::NodeDeleted { node },
            EventKind::MetaChanged { node, patch } => UiEventKind::MetaChanged {
                node,
                patch: NodeMetaPatch::from(&patch),
            },
            EventKind::GraphTransaction { transaction } => UiEventKind::GraphTransaction { transaction },
            EventKind::Custom(custom) => UiEventKind::Custom {
                topic: custom.topic,
                origin: custom.origin,
                payload: std::sync::Arc::unwrap_or_clone(custom.payload),
                retention: custom.retention,
            },
        };

        Self { time: event.time, kind }
    }
}
