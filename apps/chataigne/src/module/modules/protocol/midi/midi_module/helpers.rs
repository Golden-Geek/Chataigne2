use super::*;

pub(super) fn midi_transport_connected(
    input_selected: bool,
    input_ready: bool,
    output_selected: bool,
    output_ready: bool,
) -> bool {
    match (input_selected, output_selected) {
        (false, false) => false,
        (true, false) => input_ready,
        (false, true) => output_ready,
        (true, true) => input_ready && output_ready,
    }
}

pub(super) fn create_folder(label: &str, decl_id: &str) -> Folder {
    let mut folder = Folder::new(label);
    apply_midi_value_identity(folder.node_data_mut(), decl_id);
    folder
}

pub(super) fn create_int_parameter(
    label: &str,
    decl_id: &str,
    value: i32,
    range: Option<(i32, i32)>,
    read_only: bool,
) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Int(value), ParameterChangeCheck::ValueChange);
    apply_midi_value_identity(parameter.node_data_mut(), decl_id);
    parameter.read_only = read_only;
    parameter.constraints = int_parameter_constraints(range);
    parameter
}

pub(super) fn rotary_mode_option(variant_id: &'static str, label: &'static str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

pub(super) fn create_cc_rotary_mode_parameter(value: &'static str) -> Parameter {
    let mut parameter = Parameter::new(
        "Rotary Mode",
        ParamValue::Enum(value.to_string()),
        ParameterChangeCheck::ValueChange,
    );
    apply_midi_value_identity(parameter.node_data_mut(), MIDI_CC_ROTARY_MODE_DECL_ID);
    parameter.constraints.enum_options = (vec![
        rotary_mode_option(ROTARY_ABSOLUTE, "Absolute", 0),
        rotary_mode_option(ROTARY_TWOS_COMPLEMENT, "Two's Complement", 1),
        rotary_mode_option(ROTARY_BINARY_OFFSET, "Binary Offset", 2),
        rotary_mode_option(ROTARY_SIGN_MAGNITUDE, "Sign Magnitude", 3),
    ])
    .into();
    parameter
}

pub(super) fn create_string_parameter(label: &str, decl_id: &str, value: String, read_only: bool) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Str(value), ParameterChangeCheck::ValueChange);
    apply_midi_value_identity(parameter.node_data_mut(), decl_id);
    parameter.read_only = read_only;
    parameter
}

pub(super) fn create_trigger_parameter(label: &str, decl_id: &str) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Trigger(), ParameterChangeCheck::ValueChange);
    apply_midi_value_identity(parameter.node_data_mut(), decl_id);
    parameter
}

pub(super) fn apply_midi_value_identity(node_data: &mut golden_core::node::NodeData, decl_id: &str) {
    apply_node_identity(node_data, decl_id);
    crate::app::module::enable_module_authoring(node_data);
    node_data.meta.user_permissions = NodeUserPermissions::none();
}

pub(super) fn int_parameter_constraints(range: Option<(i32, i32)>) -> ParameterConstraints {
    match range {
        Some((min, max)) => ParameterConstraints {
            range: RangeConstraint::uniform(Some(f64::from(min)), Some(f64::from(max))),
            ..Default::default()
        },
        None => ParameterConstraints::default(),
    }
}

pub(super) fn sync_int_parameter_constraints(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    range: Option<(i32, i32)>,
    source: NodeId,
) {
    let expected = int_parameter_constraints(range);
    if snapshot.node(node_id).and_then(|node| node.param_constraints.as_ref()) == Some(&expected) {
        return;
    }

    ctx.edits.push_from(
        source,
        Edit::SetParamConstraints {
            node: node_id,
            constraints: expected,
        },
    );
}

pub(super) fn lock_midi_value_node(ctx: &mut ProcessCtx, node_id: NodeId) {
    ctx.edits.push(Edit::PatchMeta {
        node: node_id,
        patch: NodeMetaPatch {
            user_permissions: Some(NodeUserPermissions::none()),
            ..Default::default()
        },
    });
}

pub(super) fn apply_node_identity(node_data: &mut golden_core::node::NodeData, decl_id: &str) {
    node_data.meta.decl_id = DeclId(decl_id.to_string());
    node_data.meta.short_name = decl_id.to_string();
}

pub(super) fn note_from_decl_id(decl_id: &str) -> Option<u8> {
    decl_id
        .strip_prefix("note_")?
        .parse::<u8>()
        .ok()
        .filter(|value| *value <= MIDI_DATA_MAX)
}

pub(super) fn cc_from_decl_id(decl_id: &str) -> Option<u8> {
    decl_id
        .strip_prefix("cc_")?
        .parse::<u8>()
        .ok()
        .filter(|value| *value <= MIDI_DATA_MAX)
}

pub(super) fn ordered_midi_value_kind(
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
) -> Option<MidiOrderedValueKind> {
    match snapshot.node(parent_id)?.decl_id.as_str() {
        NOTES_FOLDER_DECL_ID => Some(MidiOrderedValueKind::Note),
        CONTROL_CHANGE_FOLDER_DECL_ID => Some(MidiOrderedValueKind::ControlChange),
        _ => None,
    }
}

pub(super) fn ordered_midi_value_key(kind: MidiOrderedValueKind, decl_id: &str) -> Option<u8> {
    match kind {
        MidiOrderedValueKind::Note => note_from_decl_id(decl_id),
        MidiOrderedValueKind::ControlChange => cc_from_decl_id(decl_id),
    }
}

pub(super) fn reorder_ordered_midi_value_children(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
) {
    let Some(kind) = ordered_midi_value_kind(snapshot, parent_id) else {
        return;
    };

    let mut current_children = snapshot
        .child_ids(parent_id)
        .into_iter()
        .filter_map(|child_id| {
            let key = snapshot
                .node(child_id)
                .and_then(|node| ordered_midi_value_key(kind, node.decl_id.as_str()))?;
            Some((key, child_id))
        })
        .collect::<Vec<_>>();

    let mut ordered_children = current_children.clone();
    ordered_children.sort_by_key(|(key, _child_id)| *key);

    if current_children == ordered_children {
        return;
    }

    for (_key, child_id) in ordered_children.into_iter().rev() {
        let Some(current_index) = current_children
            .iter()
            .position(|(_key, current_id)| *current_id == child_id)
        else {
            continue;
        };
        if current_index == 0 {
            continue;
        }

        let entry = current_children.remove(current_index);
        current_children.insert(0, entry);
        ctx.edits.push(Edit::MoveNode {
            node: child_id,
            new_parent: parent_id,
            new_prev_sibling: None,
        });
    }
}

pub(super) fn midi_cc_config(snapshot: &ProcessTreeSnapshot, cc_id: NodeId) -> MidiCcConfig {
    let rotary_mechanism = snapshot
        .find_child_by_decl_id(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID)
        .and_then(|mode_id| snapshot.node(mode_id))
        .and_then(|mode| mode.param_value.as_ref())
        .and_then(|value| match value {
            ParamValue::Enum(value) | ParamValue::Str(value) => normalize_rotary_mechanism(value),
            _ => None,
        })
        .unwrap_or(ROTARY_ABSOLUTE);

    MidiCcConfig { rotary_mechanism }
}

pub(super) fn normalize_rotary_mechanism(mechanism: &str) -> Option<&'static str> {
    match mechanism {
        ROTARY_ABSOLUTE => Some(ROTARY_ABSOLUTE),
        ROTARY_TWOS_COMPLEMENT => Some(ROTARY_TWOS_COMPLEMENT),
        ROTARY_BINARY_OFFSET => Some(ROTARY_BINARY_OFFSET),
        ROTARY_SIGN_MAGNITUDE => Some(ROTARY_SIGN_MAGNITUDE),
        _ => None,
    }
}

pub(super) fn merge_apply_results(current: MidiApplyResult, next: MidiApplyResult) -> MidiApplyResult {
    match (current, next) {
        (MidiApplyResult::Retry, _) | (_, MidiApplyResult::Retry) => MidiApplyResult::Retry,
        _ => MidiApplyResult::Applied,
    }
}

pub(super) fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

pub(super) fn quantize_decimal_units(value: f64, decimals: u32) -> i64 {
    let scale = 10_i64.pow(decimals.min(9));
    (value * scale as f64).round() as i64
}

pub(super) fn quantize_decimal(value: f64, decimals: u32) -> f64 {
    let scale = 10_i64.pow(decimals.min(9)) as f64;
    (value * scale).round() / scale
}

pub(super) fn stable_midi_clock_bpm(beat_durations: &VecDeque<Duration>) -> Option<f64> {
    if beat_durations.is_empty() {
        return None;
    }

    let mut samples = beat_durations.iter().map(Duration::as_secs_f64).collect::<Vec<_>>();
    samples.sort_by(f64::total_cmp);

    let trimmed = if samples.len() >= 4 {
        &samples[1..samples.len() - 1]
    } else {
        samples.as_slice()
    };
    let average_beat_seconds = trimmed.iter().sum::<f64>() / trimmed.len() as f64;
    if average_beat_seconds <= 0.0 {
        return None;
    }

    Some(60.0 / average_beat_seconds)
}

impl MtcQuarterFrameState {
    pub(super) fn apply(&mut self, raw_value: u8) {
        let part_index = usize::from((raw_value >> 4) & 0x07);
        self.nibbles[part_index] = Some(raw_value & 0x0F);
    }

    pub(super) fn frames(&self) -> i32 {
        i32::from(self.nibble(0) | ((self.nibble(1) & 0x01) << 4))
    }

    pub(super) fn seconds(&self) -> i32 {
        i32::from(self.nibble(2) | ((self.nibble(3) & 0x03) << 4))
    }

    pub(super) fn minutes(&self) -> i32 {
        i32::from(self.nibble(4) | ((self.nibble(5) & 0x03) << 4))
    }

    pub(super) fn hours(&self) -> i32 {
        i32::from(self.nibble(6) | ((self.nibble(7) & 0x01) << 4))
    }

    pub(super) fn rate_code(&self) -> i32 {
        i32::from((self.nibble(7) >> 1) & 0x03)
    }

    pub(super) fn rate_fps(&self) -> f64 {
        match self.rate_code() {
            0 => 24.0,
            1 => 25.0,
            2 => 29.97,
            3 => 30.0,
            _ => 24.0,
        }
    }

    pub(super) fn time_seconds(&self) -> f64 {
        f64::from(self.hours()) * 3600.0
            + f64::from(self.minutes()) * 60.0
            + f64::from(self.seconds())
            + (f64::from(self.frames()) / self.rate_fps())
    }

    pub(super) fn timecode_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}:{:02}",
            self.hours(),
            self.minutes(),
            self.seconds(),
            self.frames()
        )
    }

    pub(super) fn nibble(&self, index: usize) -> u8 {
        self.nibbles.get(index).copied().flatten().unwrap_or(0) & 0x0F
    }
}

pub(super) fn mtc_rate_label(rate_code: i32) -> &'static str {
    match rate_code {
        0 => "24 fps",
        1 => "25 fps",
        2 => "29.97 drop",
        3 => "30 fps",
        _ => "24 fps",
    }
}
