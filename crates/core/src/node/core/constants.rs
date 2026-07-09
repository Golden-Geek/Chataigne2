/// Built-in node type id used for user-authored lexical context scopes.
pub const USER_CONTEXT_NODE_TYPE: &str = "user_context";
/// Built-in user-item kind used for user-authored lexical context scopes.
pub const USER_CONTEXT_ITEM_KIND: &str = "user_context";
/// Default user-facing label for newly created user-context scope nodes.
pub const USER_CONTEXT_DEFAULT_LABEL: &str = "Context";
/// Built-in node type id used for folders inside user-authored lexical context scopes.
pub const USER_CONTEXT_FOLDER_NODE_TYPE: &str = "user_context_folder";
/// Built-in node type id used for multiplex axes inside user-authored lexical context scopes.
pub const USER_CONTEXT_MULTIPLEX_NODE_TYPE: &str = "user_context_multiplex";
/// Built-in user-item kind used for multiplex axes inside user-authored lexical context scopes.
pub const USER_CONTEXT_MULTIPLEX_ITEM_KIND: &str = "user_context_multiplex";
/// Default user-facing label for newly created multiplex nodes.
pub const USER_CONTEXT_MULTIPLEX_DEFAULT_LABEL: &str = "Multiplex";
/// Built-in node type prefix used for typed multiplex list nodes.
pub const USER_CONTEXT_MULTIPLEX_LIST_NODE_TYPE_PREFIX: &str = "user_context_multiplex_list_";
/// Built-in user-item kind used for typed multiplex list nodes.
pub const USER_CONTEXT_MULTIPLEX_LIST_ITEM_KIND: &str = "user_context_multiplex_list";
/// Built-in child `decl_id` for a multiplex count parameter.
pub const USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID: &str = "count";
/// Built-in folder node type id.
pub const FOLDER_NODE_TYPE: &str = "folder";
/// Built-in item kind used for parameter control helper nodes.
pub const PARAMETER_CONTROL_ITEM_KIND: &str = "parameter_control";
/// Built-in node type id for animation control nodes attached to parameters.
pub const PARAMETER_ANIMATION_CONTROL_NODE_TYPE: &str = "parameter_animation_control";
/// Built-in `decl_id` for parameter animation control nodes.
pub const PARAMETER_ANIMATION_CONTROL_DECL_ID: &str = "animation_control";
/// Built-in node type id for animation-curve container nodes.
pub const PARAMETER_ANIMATION_CURVE_NODE_TYPE: &str = "animation_curve";
/// Built-in item kind used by animation-curve container nodes.
pub const PARAMETER_ANIMATION_CURVE_ITEM_KIND: &str = "animation_curve";
/// Built-in `decl_id` for the animation-curve child under one animation control.
pub const PARAMETER_ANIMATION_CURVE_DECL_ID: &str = "curve";
/// Built-in node type id for animation-curve key nodes.
pub const PARAMETER_ANIMATION_KEY_NODE_TYPE: &str = "animation_curve_key";
/// Built-in item kind used by animation-curve key nodes.
pub const PARAMETER_ANIMATION_KEY_ITEM_KIND: &str = "animation_curve_key";
/// Built-in `decl_id` for key position parameter nodes.
pub const PARAMETER_ANIMATION_KEY_POSITION_DECL_ID: &str = "position";
/// Built-in `decl_id` for key value parameter nodes.
pub const PARAMETER_ANIMATION_KEY_VALUE_DECL_ID: &str = "value";
/// Built-in node type id for animation-curve range constraint nodes.
pub const PARAMETER_ANIMATION_RANGE_NODE_TYPE: &str = "animation_curve_range";
/// Built-in `decl_id` for range constraint nodes under animation-curve nodes.
pub const PARAMETER_ANIMATION_RANGE_DECL_ID: &str = "range";
/// Built-in `decl_id` for range x-axis bounds parameter.
pub const PARAMETER_ANIMATION_RANGE_X_DECL_ID: &str = "x";
/// Built-in `decl_id` for range y-axis bounds parameter.
pub const PARAMETER_ANIMATION_RANGE_Y_DECL_ID: &str = "y";
/// Built-in node type id for animation-curve easing nodes.
pub const PARAMETER_ANIMATION_EASING_NODE_TYPE: &str = "animation_curve_easing";
/// Built-in `decl_id` for easing nodes under key nodes.
pub const PARAMETER_ANIMATION_EASING_DECL_ID: &str = "easing";
/// Built-in `decl_id` for easing kind selector.
pub const PARAMETER_ANIMATION_EASING_KIND_DECL_ID: &str = "kind";
/// Built-in `decl_id` for bezier out-handle position.
pub const PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID: &str = "out_position";
/// Built-in `decl_id` for bezier out-handle value.
pub const PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID: &str = "out_value";
/// Built-in `decl_id` for bezier in-handle position.
pub const PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID: &str = "in_position";
/// Built-in `decl_id` for bezier in-handle value.
pub const PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID: &str = "in_value";
/// Built-in `decl_id` for steps mode selector.
pub const PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID: &str = "step_mode";
/// Built-in `decl_id` for steps size parameter.
pub const PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID: &str = "step_size";
/// Built-in `decl_id` for steps count parameter.
pub const PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID: &str = "num_steps";
/// Built-in `decl_id` for shape selector.
pub const PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID: &str = "shape";
/// Built-in `decl_id` for shape/noise amplitude parameter.
pub const PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID: &str = "amplitude";
/// Built-in `decl_id` for shape phase-mode selector.
pub const PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID: &str = "phase_mode";
/// Built-in `decl_id` for shape/noise/random frequency parameter.
pub const PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID: &str = "frequency";
/// Built-in `decl_id` for shape phase-count parameter.
pub const PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID: &str = "num_phases";
/// Built-in `decl_id` for easing fade-in parameter.
pub const PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID: &str = "fade_in";
/// Built-in `decl_id` for easing fade-out parameter.
pub const PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID: &str = "fade_out";
/// Built-in `decl_id` for perlin-noise octave count.
pub const PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID: &str = "octaves";
/// Built-in `decl_id` for perlin-noise phase parameter.
pub const PARAMETER_ANIMATION_EASING_PHASE_DECL_ID: &str = "phase";
/// Built-in `decl_id` for random seed parameter.
pub const PARAMETER_ANIMATION_EASING_SEED_DECL_ID: &str = "seed";
/// Built-in `decl_id` for script easing source.
pub const PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID: &str = "script_source";
/// Built-in child `decl_id` for control reference parameters used by `proxy`/`binding`.
pub const PARAMETER_CONTROL_REFERENCE_DECL_ID: &str = "reference";
/// Built-in child `decl_id` for expression source text on controlled parameters.
pub const PARAMETER_EXPRESSION_SOURCE_DECL_ID: &str = "expression";
/// Built-in child `decl_id` for animation waveform selector.
pub const PARAMETER_ANIMATION_WAVEFORM_DECL_ID: &str = "waveform";
/// Built-in child `decl_id` for animation frequency parameter.
pub const PARAMETER_ANIMATION_FREQUENCY_DECL_ID: &str = "frequency_hz";
/// Built-in child `decl_id` for animation amplitude parameter.
pub const PARAMETER_ANIMATION_AMPLITUDE_DECL_ID: &str = "amplitude";
/// Built-in child `decl_id` for animation offset parameter.
pub const PARAMETER_ANIMATION_OFFSET_DECL_ID: &str = "offset";
/// Built-in child `decl_id` for animation phase parameter.
pub const PARAMETER_ANIMATION_PHASE_DECL_ID: &str = "phase";
/// Built-in child `decl_id` for animation node local update rate in hertz.
pub const PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID: &str = "update_rate_hz";
/// Built-in node type id for gradient container nodes.
pub const GRADIENT_NODE_TYPE: &str = "gradient";
/// Built-in item kind used by gradient container nodes.
pub const GRADIENT_ITEM_KIND: &str = "gradient";
/// Built-in `decl_id` for the gradient child hosted inside another node.
pub const GRADIENT_DECL_ID: &str = "gradient";
/// Built-in node type id for gradient color-stop nodes.
pub const GRADIENT_STOP_NODE_TYPE: &str = "gradient_stop";
/// Built-in item kind used by gradient color-stop nodes.
pub const GRADIENT_STOP_ITEM_KIND: &str = "gradient_stop";
/// Built-in `decl_id` for the gradient stop position parameter.
pub const GRADIENT_STOP_POSITION_DECL_ID: &str = "position";
/// Built-in `decl_id` for the gradient stop color parameter.
pub const GRADIENT_STOP_COLOR_DECL_ID: &str = "color";
/// Built-in `decl_id` for the gradient stop interpolation selector.
pub const GRADIENT_STOP_INTERPOLATION_DECL_ID: &str = "interpolation";
/// All built-in parameter node type ids.
pub const PARAMETER_NODE_TYPES: [&str; 11] = [
    "trigger",
    "int",
    "float",
    "str",
    "file",
    "enum",
    "bool",
    "vec2",
    "vec3",
    "color",
    "reference",
];
pub(crate) const USER_CONTEXT_ALLOWED_ITEM_KINDS: [&str; 13] = [
    FOLDER_NODE_TYPE,
    "trigger",
    "int",
    "float",
    "str",
    "file",
    "enum",
    "bool",
    "vec2",
    "vec3",
    "color",
    "reference",
    "*",
];
