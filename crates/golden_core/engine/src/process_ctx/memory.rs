use std::{collections::HashMap, mem::size_of, sync::Arc};

use super::{ProcessTreeChildIndexes, ProcessTreeNodeSnapshot, ProcessTreeSnapshot};
use crate::{
    node::DashboardWidgetOptionsNodeKind,
    parameter::{
        ParamValue, ParameterConstraints, ParameterControlSpec, ParameterControlState, RangeConstraint, ReferenceRoot,
    },
};

fn sum(values: impl IntoIterator<Item = usize>) -> usize {
    values.into_iter().fold(0usize, usize::saturating_add)
}

fn slots<T>(capacity: usize) -> usize {
    capacity.saturating_mul(size_of::<T>())
}
fn vector<T>(value: &Vec<T>) -> usize {
    slots::<T>(value.capacity())
}
fn optional(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}
fn strings(value: &Vec<String>) -> usize {
    sum([vector(value), sum(value.iter().map(String::capacity))])
}
fn inline_only<T: Copy>(_: &T) {}
macro_rules! inline_only { ($($value:expr),+ $(,)?) => { $(inline_only($value);)+ }; }

// HashMap capacity describes usable entries rather than physical buckets. Charge ample
// bucket/control slack. This is an allocation reservation, not allocator/RSS telemetry.
fn map<K, V>(value: &HashMap<K, V>) -> usize {
    if value.capacity() == 0 {
        return 0;
    }
    value
        .capacity()
        .saturating_add(1)
        .saturating_mul(4)
        .saturating_mul(sum([size_of::<K>(), size_of::<V>(), size_of::<usize>()]))
}

pub(crate) fn arc_map<K, V>(value: &Arc<HashMap<K, V>>) -> usize {
    sum([2 * size_of::<usize>(), size_of::<HashMap<K, V>>(), map(value)])
}

fn value(value: &ParamValue) -> usize {
    value.retained_heap_bytes()
}

pub(super) fn constraints(constraints: &ParameterConstraints) -> usize {
    let ParameterConstraints {
        range,
        step,
        step_base,
        enum_options,
        policy,
        reference,
        file,
    } = constraints;
    inline_only!(step, step_base, policy);
    let range = match range {
        Some(RangeConstraint::Components { min, max }) => {
            sum([min.as_ref().map_or(0, vector), max.as_ref().map_or(0, vector)])
        }
        Some(RangeConstraint::Uniform { min, max }) => {
            inline_only!(min, max);
            0
        }
        None => 0,
    };
    let crate::parameter::ReferenceConstraints {
        root,
        target_kind,
        allowed_node_types,
        allowed_parameter_types,
        allow_projections,
        custom_filter_key,
        default_search_filter,
    } = reference;
    inline_only!(target_kind, allow_projections);
    let root = match root {
        ReferenceRoot::RelativeToOwner { path } => strings(path),
        ReferenceRoot::EngineRoot | ReferenceRoot::Uuid(_) => 0,
    };
    let crate::parameter::FileConstraints {
        allowed_types,
        allowed_extensions,
    } = file;
    if let Some(group) = allowed_types.first() {
        inline_only(group);
    }
    sum([
        range,
        enum_options.retained_bytes(),
        root,
        strings(allowed_node_types),
        strings(allowed_parameter_types),
        optional(custom_filter_key),
        optional(default_search_filter),
        vector(allowed_types),
        strings(allowed_extensions),
    ])
}

fn control(control: &ParameterControlState) -> usize {
    let ParameterControlState {
        mode,
        spec,
        diagnostics,
    } = control;
    inline_only(mode);
    let spec = match spec {
        ParameterControlSpec::ContextLink { symbol, projection } => {
            inline_only(projection);
            symbol.capacity()
        }
        ParameterControlSpec::TemplateText { template } => template.capacity(),
        ParameterControlSpec::Manual
        | ParameterControlSpec::Expression
        | ParameterControlSpec::Proxy
        | ParameterControlSpec::Binding
        | ParameterControlSpec::Animation => 0,
    };
    sum([
        spec,
        vector(diagnostics),
        sum(diagnostics.iter().map(|diagnostic| {
            let crate::parameter::ParameterControlDiagnostic { code, message, detail } = diagnostic;
            sum([code.capacity(), message.capacity(), optional(detail)])
        })),
    ])
}

fn node(node: &ProcessTreeNodeSnapshot) -> usize {
    let ProcessTreeNodeSnapshot {
        id,
        uuid,
        parent,
        first_child,
        next_sibling,
        node_type,
        decl_id,
        short_name,
        label,
        tags,
        presentation,
        enabled,
        can_be_disabled,
        child_count,
        param_value,
        param_constraints,
        param_control,
        dashboard_widget_target,
        script_properties,
        script_methods,
    } = node;
    inline_only!(
        id,
        uuid,
        parent,
        first_child,
        next_sibling,
        enabled,
        can_be_disabled,
        child_count,
    );
    let crate::node::PresentationHint {
        color,
        default_color,
        icon,
        collapsed,
        warnings,
        show_child_warnings_max_depth,
        show_in_nested_inspector,
        show_in_inspector_content,
    } = presentation;
    inline_only!(
        color,
        default_color,
        collapsed,
        show_child_warnings_max_depth,
        show_in_nested_inspector,
        show_in_inspector_content,
    );
    let warnings = sum([
        vector(warnings),
        sum(warnings.iter().map(|warning| {
            let crate::node::NodeWarning { id, message, detail } = warning;
            sum([id.capacity(), message.capacity(), optional(detail)])
        })),
    ]);
    let crate::node::DashboardWidgetTargetDescriptor {
        widget_types,
        default_widget_type_id,
    } = dashboard_widget_target;
    let widgets = sum(widget_types.iter().map(|widget| {
        let crate::node::DashboardWidgetTypeSpec {
            id,
            label,
            options_node_kind,
        } = widget;
        match options_node_kind {
            None
            | Some(
                DashboardWidgetOptionsNodeKind::Inspector
                | DashboardWidgetOptionsNodeKind::ParameterEditor
                | DashboardWidgetOptionsNodeKind::NumberSlider
                | DashboardWidgetOptionsNodeKind::NumberRotary
                | DashboardWidgetOptionsNodeKind::Vec2Pad
                | DashboardWidgetOptionsNodeKind::Vec2Editor
                | DashboardWidgetOptionsNodeKind::Vec3Editor
                | DashboardWidgetOptionsNodeKind::ColorEditor,
            ) => {}
        }
        sum([id.capacity(), label.capacity()])
    }));
    sum([
        node_type.capacity(),
        decl_id.capacity(),
        short_name.capacity(),
        label.capacity(),
        strings(tags),
        optional(icon),
        warnings,
        param_value.as_ref().map_or(0, value),
        param_constraints.as_ref().map_or(0, constraints),
        param_control.as_ref().map_or(0, control),
        vector(widget_types),
        default_widget_type_id.capacity(),
        widgets,
        map(script_properties),
        sum(script_properties
            .iter()
            .map(|(key, content)| sum([key.capacity(), value(content)]))),
        strings(script_methods),
    ])
}

fn aliases(maps: &HashMap<crate::node::NodeId, HashMap<Box<str>, crate::node::NodeId>>) -> usize {
    sum([
        map(maps),
        sum(maps
            .values()
            .map(|entries| sum([map(entries), sum(entries.keys().map(|key| key.len()))]))),
    ])
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AllocationWeights {
    pub(super) nodes: usize,
    pub(super) overlays: usize,
    pub(super) uuids: usize,
    pub(super) children: usize,
    pub(super) revisions: usize,
}

impl AllocationWeights {
    pub(super) fn total(self) -> usize {
        sum([self.nodes, self.overlays, self.uuids, self.children, self.revisions])
    }
}

pub(super) fn weights(snapshot: &ProcessTreeSnapshot) -> AllocationWeights {
    let indexes: &ProcessTreeChildIndexes = &snapshot.child_indexes;
    let ProcessTreeChildIndexes {
        ids_by_parent,
        ids_by_decl_id,
        ids_by_child_key,
    } = indexes;
    AllocationWeights {
        nodes: sum([
            arc_map(&snapshot.nodes),
            // Index slack contains pointers. Every populated bucket additionally owns
            // one complete record allocation, including conservative allocator overhead.
            slots::<ProcessTreeNodeSnapshot>(snapshot.nodes.len()),
            snapshot.nodes.len().saturating_mul(2 * size_of::<usize>()),
            sum(snapshot.nodes.values().map(|record| node(record))),
        ]),
        overlays: overlays(&snapshot.node_overrides),
        uuids: arc_map(&snapshot.node_ids_by_uuid),
        revisions: snapshot.revisions.retained_heap_bytes(),
        children: sum([
            2 * size_of::<usize>(),
            size_of::<ProcessTreeChildIndexes>(),
            map(ids_by_parent),
            sum(ids_by_parent
                .values()
                .map(|ids| slots::<crate::node::NodeId>(ids.len()))),
            aliases(ids_by_decl_id),
            aliases(ids_by_child_key),
        ]),
    }
}

pub(super) fn overlays(nodes: &Arc<HashMap<crate::node::NodeId, ProcessTreeNodeSnapshot>>) -> usize {
    sum([arc_map(nodes), sum(nodes.values().map(node))])
}
