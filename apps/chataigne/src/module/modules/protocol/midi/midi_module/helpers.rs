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

pub(super) fn feedback_messages_for_param(
    snapshot: &ProcessTreeSnapshot,
    param: NodeId,
    old_value: ParamValue,
) -> Option<Vec<MidiMessage>> {
    let param_snapshot = snapshot.node(param)?;
    let parent_id = param_snapshot.parent?;
    let parent = snapshot.node(parent_id)?;

    if parent.decl_id.as_str() == NOTES_FOLDER_DECL_ID {
        return feedback_messages_for_note_param(snapshot, param_snapshot);
    }

    if parent.decl_id.as_str() == CONTROL_CHANGE_FOLDER_DECL_ID {
        return feedback_messages_for_cc_param(snapshot, param_snapshot, old_value);
    }

    if parent.decl_id.as_str() == POLY_PRESSURE_FOLDER_DECL_ID {
        let channel = channel_for_descendant(snapshot, parent_id)?;
        let note = note_from_decl_id(param_snapshot.decl_id.as_str())?;
        let pressure = param_snapshot.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?;
        return Some(vec![MidiMessage::PolyPressure {
            channel,
            note,
            pressure,
        }]);
    }

    let channel = channel_for_descendant(snapshot, param)?;
    match param_snapshot.decl_id.as_str() {
        PROGRAM_DECL_ID => Some(vec![MidiMessage::ProgramChange {
            channel,
            program: param_snapshot.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        }]),
        CHANNEL_PRESSURE_DECL_ID => Some(vec![MidiMessage::ChannelPressure {
            channel,
            pressure: param_snapshot.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        }]),
        PITCH_BEND_DECL_ID => Some(vec![MidiMessage::PitchBend {
            channel,
            value: param_snapshot
                .param_value
                .as_ref()?
                .as_int()
                .map(clamp_i32_to_u14)
                .unwrap_or(MIDI_PITCH_BEND_CENTER),
        }]),
        _ => feedback_messages_for_system_param(param_snapshot),
    }
}

pub(super) fn feedback_messages_for_note_param(
    snapshot: &ProcessTreeSnapshot,
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
) -> Option<Vec<MidiMessage>> {
    let channel = channel_for_descendant(snapshot, param.id)?;
    let note = note_from_decl_id(param.decl_id.as_str())?;
    let velocity = param.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?;

    Some(vec![if velocity == 0 {
        MidiMessage::NoteOff {
            channel,
            note,
            velocity: 0,
        }
    } else {
        MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        }
    }])
}

pub(super) fn feedback_messages_for_cc_param(
    snapshot: &ProcessTreeSnapshot,
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
    old_value: ParamValue,
) -> Option<Vec<MidiMessage>> {
    let channel = channel_for_descendant(snapshot, param.id)?;
    let controller = cc_from_decl_id(param.decl_id.as_str())?;
    let value = param.param_value.as_ref()?.as_int().unwrap_or(0);
    let config = midi_cc_config(snapshot, param.id);
    let channel_14_bit = midi_cc_channel_is_14_bit(snapshot, param.id, channel);

    if config.rotary_mechanism != ROTARY_ABSOLUTE {
        let old = old_value.as_int().unwrap_or(value);
        let delta = value.saturating_sub(old);
        let raw_delta = encode_rotary_delta(config.rotary_mechanism, delta)?;
        return Some(vec![MidiMessage::ControlChange {
            channel,
            controller,
            value: raw_delta,
        }]);
    }

    if channel_14_bit && cc_supports_14_bit(controller) {
        return Some(encode_14_bit_control_change(
            channel,
            controller,
            clamp_i32_to_u14(value),
        ));
    }

    if channel_14_bit && (32..=63).contains(&controller) {
        return None;
    }

    Some(vec![MidiMessage::ControlChange {
        channel,
        controller,
        value: clamp_i32_to_u7(value),
    }])
}

pub(super) fn feedback_messages_for_system_param(
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
) -> Option<Vec<MidiMessage>> {
    let system = match param.decl_id.as_str() {
        TIME_CODE_QUARTER_FRAME_DECL_ID => MidiSystemMessage::TimeCodeQuarterFrame {
            value: param.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        },
        SONG_POSITION_DECL_ID => MidiSystemMessage::SongPosition {
            position: param.param_value.as_ref()?.as_int().map(clamp_i32_to_u14)?,
        },
        SONG_SELECT_DECL_ID => MidiSystemMessage::SongSelect {
            song: param.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        },
        TUNE_REQUEST_DECL_ID => MidiSystemMessage::TuneRequest,
        TIMING_CLOCK_DECL_ID => MidiSystemMessage::TimingClock,
        START_DECL_ID => MidiSystemMessage::Start,
        CONTINUE_DECL_ID => MidiSystemMessage::Continue,
        STOP_DECL_ID => MidiSystemMessage::Stop,
        ACTIVE_SENSING_DECL_ID => MidiSystemMessage::ActiveSensing,
        RESET_DECL_ID => MidiSystemMessage::Reset,
        _ => return None,
    };
    Some(vec![MidiMessage::System(system)])
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
) {
    let expected = int_parameter_constraints(range);
    if snapshot.node(node_id).and_then(|node| node.param_constraints.as_ref()) == Some(&expected) {
        return;
    }

    ctx.edits.push(Edit::SetParamConstraints {
        node: node_id,
        constraints: expected,
    });
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

pub(super) fn channel_for_descendant(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<u8> {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        let node = snapshot.node(current_id)?;
        if let Some(channel) = node.decl_id.strip_prefix("channel_") {
            return channel.parse::<u8>().ok().filter(|value| (1..=16).contains(value));
        }
        current = node.parent;
    }
    None
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

pub(super) fn is_descendant_of(snapshot: &ProcessTreeSnapshot, start: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(start);
    while let Some(node_id) = current {
        if node_id == ancestor {
            return true;
        }
        current = snapshot.node(node_id).and_then(|node| node.parent);
    }
    false
}

pub(super) fn enclosing_midi_module(snapshot: &ProcessTreeSnapshot, start: NodeId) -> Option<NodeId> {
    let mut current = Some(start);
    while let Some(node_id) = current {
        let node = snapshot.node(node_id)?;
        if node.node_type == MidiModule::NODE_TYPE {
            return Some(node_id);
        }
        current = node.parent;
    }
    None
}

pub(super) fn midi_cc_channel_is_14_bit(snapshot: &ProcessTreeSnapshot, start: NodeId, channel: u8) -> bool {
    let Some(module_id) = enclosing_midi_module(snapshot, start) else {
        return false;
    };
    let Some(parameters_id) = snapshot.find_child_by_decl_id(module_id, "parameters") else {
        return false;
    };
    let Some(folder_id) = snapshot.find_child_by_decl_id(parameters_id, FOURTEEN_BIT_CHANNELS_FOLDER_DECL_ID) else {
        return false;
    };
    snapshot
        .find_child_by_decl_id(folder_id, channel_decl_id(channel).as_str())
        .and_then(|param_id| snapshot.node(param_id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_bool)
        .unwrap_or(false)
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
