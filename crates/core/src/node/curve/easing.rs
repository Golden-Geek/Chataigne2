use super::helpers::{
    curve_easing_kind_id, curve_phase_mode_variant_id, curve_shape_variant_id, curve_step_mode_variant_id,
    parse_phase_mode, parse_shape, parse_step_mode, read_child_param_enum, read_child_param_f64,
    read_child_param_string, read_child_param_u32, read_child_param_u64,
};
use super::prelude::*;

/// Internal node storing one key-to-next easing specification.
#[allow(missing_docs)]
#[crate::node("animation_curve_easing")]
#[children(
    kind: crate::parameter::Enum = "bezier" (
        label = "Kind",
        enum_options = ["linear", "bezier", "hold", "steps", "shape", "perlinNoise", "random", "script"],
    );
    out_position: f64 = 1.0 / 3.0 (
        label = "Out Handle Position",
        dependency = kind == "bezier",
    );
    out_value: f64 = 0.0 (
        label = "Out Handle Value",
        dependency = kind == "bezier",
    );
    in_position: f64 = -1.0 / 3.0 (
        label = "In Handle Position",
        dependency = kind == "bezier",
    );
    in_value: f64 = 0.0 (
        label = "In Handle Value",
        dependency = kind == "bezier",
    );
    step_mode: crate::parameter::Enum = "numSteps" (
        label = "Step Mode",
        enum_options = ["stepSize", "numSteps"],
        dependency = kind == "steps",
    );
    step_size: f64 = 0.1 [0.0..] (
        label = "Step Size",
        dependency = kind == "steps" && step_mode == "stepSize",
    );
    num_steps: i32 = 8 [1..] (
        label = "Number of Steps",
        dependency = kind == "steps" && step_mode == "numSteps",
    );
    shape: crate::parameter::Enum = "sine" (
        label = "Shape",
        enum_options = ["sine", "triangle", "saw", "reverseSaw", "square"],
        dependency = kind == "shape",
    );
    amplitude: f64 = 1.0 (
        label = "Amplitude",
        dependency = kind == "shape" || kind == "perlinNoise",
    );
    phase_mode: crate::parameter::Enum = "frequency" (
        label = "Phase Mode",
        enum_options = ["frequency", "numPhases"],
        dependency = kind == "shape",
    );
    frequency: f64 = 1.0 [0.0..] (
        label = "Frequency",
        dependency = kind == "shape" || kind == "perlinNoise" || kind == "random",
    );
    num_phases: f64 = 1.0 [0.0..] (
        label = "Number of Phases",
        dependency = kind == "shape" && phase_mode == "numPhases",
    );
    fade_in: f64 = 0.0 [0.0..] (
        label = "Fade In",
        dependency = kind == "shape" || kind == "perlinNoise" || kind == "random",
    );
    fade_out: f64 = 0.0 [0.0..] (
        label = "Fade Out",
        dependency = kind == "shape" || kind == "perlinNoise" || kind == "random",
    );
    octaves: i32 = 4 [1..] (
        label = "Octaves",
        dependency = kind == "perlinNoise",
    );
    phase: f64 = 0.0 (
        label = "Phase",
        dependency = kind == "perlinNoise",
    );
    seed: i32 = 0 (
        label = "Seed",
        dependency = kind == "random",
    );
    script_source: String = "".to_string() (
        label = "Script Source",
        dependency = kind == "script",
    );
)]
pub struct CurveEasingNode {}

impl CurveEasingNode {
    /// Creates one easing node with explicit default easing values.
    pub fn new_with_easing(label: impl Into<String>, default_easing: CurveEasing) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_EASING_DECL_ID.to_string());

        let (
            kind,
            out_position,
            out_value,
            in_position,
            in_value,
            step_mode,
            step_size,
            num_steps,
            shape,
            amplitude,
            phase_mode,
            frequency,
            num_phases,
            fade_in,
            fade_out,
            octaves,
            phase,
            seed,
            script_source,
        ) = Self::defaults_from_easing(&default_easing);

        Self {
            node_data,
            kind: crate::node::ParameterHandle::new(kind.into()),
            out_position: crate::node::ParameterHandle::new(out_position),
            out_value: crate::node::ParameterHandle::new(out_value),
            in_position: crate::node::ParameterHandle::new(in_position),
            in_value: crate::node::ParameterHandle::new(in_value),
            step_mode: crate::node::ParameterHandle::new(step_mode.into()),
            step_size: crate::node::ParameterHandle::new(step_size),
            num_steps: crate::node::ParameterHandle::new(num_steps),
            shape: crate::node::ParameterHandle::new(shape.into()),
            amplitude: crate::node::ParameterHandle::new(amplitude),
            phase_mode: crate::node::ParameterHandle::new(phase_mode.into()),
            frequency: crate::node::ParameterHandle::new(frequency),
            num_phases: crate::node::ParameterHandle::new(num_phases),
            fade_in: crate::node::ParameterHandle::new(fade_in),
            fade_out: crate::node::ParameterHandle::new(fade_out),
            octaves: crate::node::ParameterHandle::new(octaves),
            phase: crate::node::ParameterHandle::new(phase),
            seed: crate::node::ParameterHandle::new(seed),
            script_source: crate::node::ParameterHandle::new(script_source),
        }
    }

    fn defaults_from_easing(
        easing: &CurveEasing,
    ) -> (
        &'static str,
        f64,
        f64,
        f64,
        f64,
        &'static str,
        f64,
        i32,
        &'static str,
        f64,
        &'static str,
        f64,
        f64,
        f64,
        f64,
        i32,
        f64,
        i32,
        String,
    ) {
        let kind = curve_easing_kind_id(easing);
        let out_position = match easing {
            CurveEasing::Bezier { out_handle, .. } => out_handle.position,
            _ => 1.0 / 3.0,
        };
        let out_value = match easing {
            CurveEasing::Bezier { out_handle, .. } => out_handle.value,
            _ => 0.0,
        };
        let in_position = match easing {
            CurveEasing::Bezier { in_handle, .. } => in_handle.position,
            _ => -1.0 / 3.0,
        };
        let in_value = match easing {
            CurveEasing::Bezier { in_handle, .. } => in_handle.value,
            _ => 0.0,
        };
        let step_mode = match easing {
            CurveEasing::Steps { step_mode, .. } => curve_step_mode_variant_id(*step_mode),
            _ => "numSteps",
        };
        let step_size = match easing {
            CurveEasing::Steps { step_size, .. } => *step_size,
            _ => 0.1,
        };
        let num_steps = match easing {
            CurveEasing::Steps { num_steps, .. } => (*num_steps).max(1) as i32,
            _ => 8,
        };
        let shape = match easing {
            CurveEasing::Shape { shape, .. } => curve_shape_variant_id(*shape),
            _ => "sine",
        };
        let amplitude = match easing {
            CurveEasing::Shape { amplitude, .. } | CurveEasing::PerlinNoise { amplitude, .. } => *amplitude,
            _ => 1.0,
        };
        let phase_mode = match easing {
            CurveEasing::Shape { phase_mode, .. } => curve_phase_mode_variant_id(*phase_mode),
            _ => "frequency",
        };
        let frequency = match easing {
            CurveEasing::Shape { frequency, .. }
            | CurveEasing::PerlinNoise { frequency, .. }
            | CurveEasing::Random { frequency, .. } => *frequency,
            _ => 1.0,
        };
        let num_phases = match easing {
            CurveEasing::Shape { num_phases, .. } => *num_phases,
            _ => 1.0,
        };
        let fade_in = match easing {
            CurveEasing::Shape { fade_in, .. }
            | CurveEasing::PerlinNoise { fade_in, .. }
            | CurveEasing::Random { fade_in, .. } => *fade_in,
            _ => 0.0,
        };
        let fade_out = match easing {
            CurveEasing::Shape { fade_out, .. }
            | CurveEasing::PerlinNoise { fade_out, .. }
            | CurveEasing::Random { fade_out, .. } => *fade_out,
            _ => 0.0,
        };
        let octaves = match easing {
            CurveEasing::PerlinNoise { octaves, .. } => (*octaves).max(1) as i32,
            _ => 4,
        };
        let phase = match easing {
            CurveEasing::PerlinNoise { phase, .. } => *phase,
            _ => 0.0,
        };
        let seed = match easing {
            CurveEasing::Random { seed, .. } => (*seed).min(i32::MAX as u64) as i32,
            _ => 0,
        };
        let script_source = match easing {
            CurveEasing::Script { source } => source.clone(),
            _ => String::new(),
        };

        (
            kind,
            out_position,
            out_value,
            in_position,
            in_value,
            step_mode,
            step_size,
            num_steps,
            shape,
            amplitude,
            phase_mode,
            frequency,
            num_phases,
            fade_in,
            fade_out,
            octaves,
            phase,
            seed,
            script_source,
        )
    }
}

#[crate::node("animation_curve_easing", from_struct)]
impl Node for CurveEasingNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.can_be_disabled = false;
        self.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_EASING_DECL_ID.to_string());
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}

pub(super) fn parse_easing_from_snapshot(snapshot: &ProcessTreeSnapshot, easing_node: NodeId) -> CurveEasing {
    let kind = read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID, "linear");
    match kind.trim().to_ascii_lowercase().as_str() {
        "bezier" => CurveEasing::Bezier {
            out_handle: CurveHandle::new(
                read_child_param_f64(
                    snapshot,
                    easing_node,
                    PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID,
                    1.0 / 3.0,
                ),
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, 0.0),
            ),
            in_handle: CurveHandle::new(
                read_child_param_f64(
                    snapshot,
                    easing_node,
                    PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID,
                    -1.0 / 3.0,
                ),
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, 0.0),
            ),
        },
        "hold" => CurveEasing::Hold,
        "steps" => CurveEasing::Steps {
            step_mode: parse_step_mode(
                read_child_param_enum(
                    snapshot,
                    easing_node,
                    PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID,
                    "numSteps",
                )
                .as_str(),
            ),
            step_size: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, 0.1),
            num_steps: read_child_param_u32(snapshot, easing_node, PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, 8)
                .max(1),
        },
        "shape" => CurveEasing::Shape {
            shape: parse_shape(
                read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, "sine").as_str(),
            ),
            amplitude: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0),
            phase_mode: parse_phase_mode(
                read_child_param_enum(
                    snapshot,
                    easing_node,
                    PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID,
                    "frequency",
                )
                .as_str(),
            ),
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0),
            num_phases: read_child_param_f64(
                snapshot,
                easing_node,
                PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID,
                1.0,
            ),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
        },
        "perlinnoise" => CurveEasing::PerlinNoise {
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0),
            amplitude: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0),
            octaves: read_child_param_u32(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, 4).max(1),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
            phase: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_PHASE_DECL_ID, 0.0),
        },
        "random" => CurveEasing::Random {
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 6.0),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
            seed: read_child_param_u64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SEED_DECL_ID, 0),
        },
        "script" => CurveEasing::Script {
            source: read_child_param_string(
                snapshot,
                easing_node,
                PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID,
                "",
            ),
        },
        _ => CurveEasing::Linear,
    }
}
