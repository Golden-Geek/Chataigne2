export type NodeId = number;

export type ParamEventBehaviour = 'Coalesce' | 'Append';
export type ParamConstraintPolicy = 'ClampAdapt' | 'Reject';
export type UiAckStatus = 'applied' | 'staged' | 'rejected';
export type UiLogLevel = 'info' | 'warning' | 'error' | 'success';
export type UiParameterControlMode =
	| 'manual'
	| 'contextLink'
	| 'templateText'
	| 'expression'
	| 'proxy'
	| 'binding'
	| 'animation';

export type CssUnit = 'px' | 'rem' | 'em' | 'percent' | 'vw' | 'vh';

export type ParamValue =
	| { kind: 'trigger' }
	| { kind: 'int'; value: number }
	| { kind: 'float'; value: number }
	| { kind: 'str'; value: string }
	| { kind: 'file'; value: string }
	| { kind: 'enum'; value: string }
	| { kind: 'bool'; value: boolean }
	| { kind: 'css_value'; value: number; unit: CssUnit }
	| { kind: 'vec2'; value: [number, number] }
	| { kind: 'vec3'; value: [number, number, number] }
	| { kind: 'color'; value: [number, number, number, number] }
	| {
			kind: 'reference';
			uuid: string;
			projection?: UiParamValueProjection;
			cached_id?: NodeId;
			cached_name?: string;
			relative_path_from_root?: string[];
	  };

export type UiReferenceRoot =
	| { kind: 'engineRoot' }
	| { kind: 'uuid'; uuid: string }
	| { kind: 'relativeToOwner'; path: string[] };

export type UiReferenceTargetKind = 'anyNode' | 'parameterOnly';

export type UiFileTypeGroup = 'audio' | 'video' | 'script';

export interface UiFileConstraints {
	allowed_types: UiFileTypeGroup[];
	allowed_extensions: string[];
}

export interface UiReferenceConstraints {
	root: UiReferenceRoot;
	target_kind: UiReferenceTargetKind;
	allowed_node_types: string[];
	allowed_parameter_types: string[];
	allow_projections: boolean;
	custom_filter_key?: string;
	default_search_filter?: string;
}

export type UiScriptSource =
	| { kind: 'inline'; text: string }
	| { kind: 'projectFile'; path: string };

export interface UiScriptConfig {
	source: UiScriptSource;
}

export interface UiScriptFnSignature {
	args: string[];
	returns?: string;
}

export interface UiScriptExportSpec {
	name: string;
	signature: UiScriptFnSignature;
}

export interface UiScriptManifest {
	api_version: number;
	update_rate_hz?: number;
	exports: UiScriptExportSpec[];
}

export interface UiScriptState {
	config: UiScriptConfig;
	effective_update_rate_hz?: number;
	export_names: string[];
	manifest?: UiScriptManifest;
}

export type UiSubscriptionScope =
	| { kind: 'wholeGraph' }
	| { kind: 'subtree'; root: NodeId; max_depth: number };

export type UiUserNodeRole = 'regular' | 'itemRoot';

export interface EventTime {
	tick: number;
	micro: number;
	seq: number;
}

export interface UiNodeWarningDto {
	id?: string;
	message: string;
	detail?: string;
}

export interface UiColorDto {
	r: number;
	g: number;
	b: number;
	a: number;
}

export interface UiNodePresentationHintsDto {
	// User-selected UI color override.
	color?: UiColorDto;
	// Backend-provided default UI color for this node kind or declaration.
	default_color?: UiColorDto;
	// Preferred UI icon, as a data URI (e.g. `data:image/svg+xml;base64,...`).
	icon?: string;
	// Default collapsed state for tree/inspector containers until the user toggles them.
	collapsed?: boolean;
	// Warnings that belong to this node.
	warnings?: UiNodeWarningDto[];
	// If > 0, this node also surfaces warnings from descendants up to this depth.
	show_child_warnings_max_depth?: number;
	// Defaults to visible; set false to hide this item unless selected directly.
	show_in_nested_inspector?: boolean;
	// Defaults to visible; set false when a custom inspector renders this node elsewhere.
	show_in_inspector_content?: boolean;
}

export interface UiNodeUserPermissionsDto {
	can_edit_name: boolean;
	can_remove_and_duplicate: boolean;
	can_edit_constraints: boolean;
	can_edit_tags: boolean;
	can_edit_color: boolean;
}

export interface UiNodeMetaDto {
	short_name: string;
	label: string;
	enabled: boolean;
	can_be_disabled: boolean;
	user_permissions: UiNodeUserPermissionsDto;
	description?: string;
	declared_description_key?: string;
	description_overridden?: boolean;
	tags: string[];
	presentation?: UiNodePresentationHintsDto;
}

export interface ParamEnumOption {
	variant_id: string;
	value: ParamValue;
	label: string;
	tags: string[];
	ordering?: number;
}

export type UiRangeConstraint =
	| { kind: 'uniform'; min?: number; max?: number }
	| { kind: 'components'; min?: number[]; max?: number[] };

export interface UiParamConstraints {
	range?: UiRangeConstraint;
	step?: number;
	step_base?: number;
	enum_options: ParamEnumOption[];
	policy: ParamConstraintPolicy;
	reference?: UiReferenceConstraints;
	file?: UiFileConstraints;
}

export interface UiParamHints {
	widget?: string;
	unit?: string;
}

export type UiUserContextValueType =
	| 'trigger'
	| 'int'
	| 'float'
	| 'str'
	| 'file'
	| 'enum'
	| 'bool'
	| 'css_value'
	| 'vec2'
	| 'vec3'
	| 'color'
	| 'reference';

export type UiParamValueProjection =
	| 'floatToVec2X0'
	| 'floatToVec20Y'
	| 'floatToVec2XX'
	| 'floatToVec3X00'
	| 'floatToVec30Y0'
	| 'floatToVec300Z'
	| 'floatToVec3XXX'
	| 'vec2X'
	| 'vec2Y'
	| 'vec2ToVec3XY0'
	| 'vec2ToVec3X0Y'
	| 'vec2ToColorHs'
	| 'vec3X'
	| 'vec3Y'
	| 'vec3Z'
	| 'vec3ToVec2XY'
	| 'vec3ToVec2XZ'
	| 'vec3ToVec2YZ'
	| 'vec3ToColorRgb'
	| 'vec3ToColorHsv'
	| 'colorR'
	| 'colorG'
	| 'colorB'
	| 'colorA'
	| 'colorToVec3Rgb'
	| 'colorToVec3Hsv'
	| 'colorToVec2Hs';

export interface UiUserContextCandidate {
	symbol: string;
	value_type: UiUserContextValueType;
	kind: 'scalar' | 'multiplexList';
	multiplex?: UiUserContextMultiplexList;
	scope_owner: NodeId;
	lexical_depth: number;
	entry_param: NodeId;
	compatible: boolean;
	directly_compatible: boolean;
	multiplex_index_compatible: boolean;
	shadowed: boolean;
	projections: UiParamValueProjection[];
}

export interface UiUserContextMultiplexList {
	multiplex: NodeId;
	list: NodeId;
	axis_id: string;
	index_link_symbol: string;
	index0_link_symbol: string;
	list_link_symbol: string;
}

export interface UiTokenSuggestion {
	token: string;
}

export interface UiParamControlCandidate {
	param: NodeId;
	compatible: boolean;
	projections: UiParamValueProjection[];
}

export interface UiParamControlInfo {
	param: NodeId;
	active_mode: UiParameterControlMode;
	available_modes: UiParameterControlMode[];
	context_candidates: UiUserContextCandidate[];
	token_suggestions: UiTokenSuggestion[];
	proxy_candidates: UiParamControlCandidate[];
	binding_candidates: UiParamControlCandidate[];
}

export interface UiNodeReferenceValue {
	uuid: string;
	projection?: UiParamValueProjection;
	cached_id?: NodeId;
	cached_name?: string;
	relative_path_from_root?: string[];
}

export interface UiAnimationControlSpec {
	waveform: 'sine' | 'triangle' | 'saw' | 'square';
	frequency_hz: number;
	amplitude: number;
	offset: number;
	phase: number;
}

export type UiParameterControlSpec =
	| { mode: 'manual' }
	| { mode: 'contextLink'; symbol: string; projection?: UiParamValueProjection }
	| { mode: 'templateText'; template: string }
	| { mode: 'expression' }
	| { mode: 'proxy' }
	| { mode: 'binding' }
	| { mode: 'animation' };

export interface UiParameterControlState {
	mode: UiParameterControlMode;
	spec: UiParameterControlSpec;
}

export interface UiParamDto {
	value: ParamValue;
	default_value: ParamValue;
	event_behaviour: ParamEventBehaviour;
	read_only: boolean;
	constraints: UiParamConstraints;
	ui_hints: UiParamHints;
	control: UiParameterControlState;
	reference_allowed_targets: NodeId[];
	reference_visible_nodes: NodeId[];
}

export interface UiReferenceTargets {
	allowed_target_ids: NodeId[];
	visible_node_ids: NodeId[];
	candidates: UiReferenceTargetCandidate[];
}

export interface UiReferenceTargetCandidate {
	target_id: NodeId;
	direct: boolean;
	projections: UiParamValueProjection[];
}

export type UiNodeDataDto =
	| { kind: 'parameter'; param: UiParamDto }
	| { kind: 'node'; node_type: string };

export interface UiNodeDto {
	node_id: NodeId;
	uuid: string;
	decl_id: string;
	node_type: string;
	meta: UiNodeMetaDto;
	data: UiNodeDataDto;
	user_role: UiUserNodeRole;
	user_item_kind: string;
	accepted_user_item_kinds: string[];
	creatable_user_items: UiCreatableUserItem[];
	children: NodeId[];
}

export interface UiCreatableUserItem {
	node_type: string;
	item_kind: string;
	label: string;
	menu_path: string[];
	initial_params: UiCreateUserItemInitialParam[];
	menu_path_colors?: string[];
	color?: string;
	icon?: string;
	select_when_created: boolean;
	separator_before?: boolean;
}

export interface UiNodeTypeDescriptor {
	node_type: string;
	description?: string;
}

export interface UiDeclaredDescriptionDescriptor {
	key: string;
	description: string;
}

export interface UiSchemaView {
	node_types: UiNodeTypeDescriptor[];
	declared_descriptions: UiDeclaredDescriptionDescriptor[];
	enums: Array<{
		enum_id: string;
		variants: Array<{
			variant_id: string;
			value: ParamValue;
			label: string;
			tags: string[];
			ordering?: number;
		}>;
	}>;
}

export interface UiHistoryState {
	can_undo: boolean;
	can_redo: boolean;
	undo_len: number;
	redo_len: number;
	active_edit_session: boolean;
	current_history_state_id: number;
}

export interface UiLogRecord {
	id: number;
	timestamp_ms: number;
	level: UiLogLevel;
	tag: string;
	message: string;
	repeat_count?: number;
	origin?: NodeId;
}

export interface UiLoggerState {
	max_entries: number;
	records: UiLogRecord[];
}

export interface UiProjectFileSpec {
	display_name: string;
	extension: string;
	current_path: string | null;
}

export interface UiSnapshot {
	protocol_version: string;
	scope: UiSubscriptionScope;
	at: EventTime;
	nodes: UiNodeDto[];
	schema: UiSchemaView;
	history: UiHistoryState;
	logger: UiLoggerState;
	project_file: UiProjectFileSpec;
}

export interface UiChildrenOrderPatch {
	parent: NodeId;
	children: NodeId[];
}

export type UiGraphOp =
	| { kind: 'nodeCreated'; snapshot: UiNodeDto; parent?: NodeId | null; index?: number | null }
	| {
			kind: 'subtreeInserted';
			root: NodeId;
			parent: NodeId;
			nodes: UiNodeDto[];
			parent_children_after: NodeId[];
	  }
	| {
			kind: 'subtreeRemoved';
			root: NodeId;
			removed_ids: NodeId[];
			parent_after?: UiChildrenOrderPatch | null;
	  }
	| {
			kind: 'nodeMoved';
			node: NodeId;
			old_parent?: NodeId | null;
			new_parent?: NodeId | null;
			old_parent_after?: UiChildrenOrderPatch | null;
			new_parent_after?: UiChildrenOrderPatch | null;
	  }
	| { kind: 'childrenReordered'; parent: NodeId; children: NodeId[] }
	| { kind: 'nodeMetaPatched'; node: NodeId; patch: Partial<UiNodeMetaDto> }
	| {
			kind: 'paramPatched';
			node: NodeId;
			param: NodeId;
			patch: {
				value?: ParamValue;
				control?: UiParameterControlState;
				constraints?: UiParamConstraints;
			};
	  }
	| { kind: 'historyPatched'; history: UiHistoryState }
	| { kind: 'loggerPatched'; records_added: UiLogRecord[]; dropped_before?: number | null };

export interface UiGraphTransaction {
	tx_id: number;
	epoch: number;
	base_graph_version: number;
	next_graph_version: number;
	ops: UiGraphOp[];
}

export type UiEventKind =
	| ({ kind: 'graphTransaction' } & UiGraphTransaction)
	| { kind: 'paramChanged'; param: NodeId; old_value: ParamValue; new_value: ParamValue }
	| {
			kind: 'paramControlChanged';
			param: NodeId;
			old_state: UiParameterControlState;
			new_state: UiParameterControlState;
	  }
	| {
			kind: 'paramConstraintsChanged';
			param: NodeId;
			old_constraints: UiParamConstraints;
			new_constraints: UiParamConstraints;
	  }
	| {
			kind: 'childAdded';
			parent: NodeId;
			child: NodeId;
			decl_id: string;
			parent_children?: NodeId[] | null;
	  }
	| { kind: 'childRemoved'; parent: NodeId; child: NodeId }
	| { kind: 'childReplaced'; parent: NodeId; old: NodeId; new: NodeId; decl_id: string }
	| {
			kind: 'childMoved';
			child: NodeId;
			old_parent: NodeId;
			new_parent: NodeId;
			old_parent_children?: NodeId[] | null;
			new_parent_children?: NodeId[] | null;
	  }
	| { kind: 'childReordered'; parent: NodeId; child: NodeId; parent_children?: NodeId[] | null }
	| { kind: 'nodeCreated'; node: NodeId; snapshot?: UiNodeDto | null }
	| { kind: 'nodeDeleted'; node: NodeId }
	| { kind: 'metaChanged'; node: NodeId; patch: Partial<UiNodeMetaDto> }
	| { kind: 'custom'; topic: string; origin?: NodeId; payload: unknown };

export interface UiEventDto {
	time: EventTime;
	kind: UiEventKind;
}

export interface UiEventBatch {
	from?: EventTime;
	to?: EventTime;
	runtime?: UiRuntimeStats;
	events: UiEventDto[];
}

export interface UiRuntimeStats {
	engine_hz: number;
}

export interface UiAnimationCurveFitPoint {
	position: number;
	value: number;
}

export interface UiAnimationCurveBezierFitOptions {
	max_value_error: number;
	max_keys: number;
}

export interface UiCreateUserItemInitialParam {
	decl_id: string;
	value: ParamValue;
}

export interface UiDashboardWidgetSizeEnabled {
	width: boolean;
	height: boolean;
}

export interface UiDashboardWidgetCssValue {
	value: number;
	unit: CssUnit;
}

export interface UiDashboardWidgetPlacement {
	anchor: string;
	position: [number, number];
	width: UiDashboardWidgetCssValue;
	height: UiDashboardWidgetCssValue;
	size_enabled?: UiDashboardWidgetSizeEnabled;
}

export interface UiDuplicateNodeSpec {
	source: NodeId;
	new_parent: NodeId;
	new_prev_sibling?: NodeId;
	label?: string;
	initial_params?: UiCreateUserItemInitialParam[];
}

export interface UiDuplicateCreateUserItemSpec {
	source: NodeId;
	parent: NodeId;
	node_type: string;
	label?: string;
	initial_params?: UiCreateUserItemInitialParam[];
}

export type UiDuplicateDependentInitialParamValue =
	| { kind: 'literal'; value: ParamValue }
	| { kind: 'duplicatedNodeReference'; source: NodeId };

export interface UiDuplicateDependentUserItemInitialParam {
	decl_id: string;
	value: UiDuplicateDependentInitialParamValue;
}

export interface UiDuplicateDependentUserItem {
	parent: NodeId;
	node_type: string;
	label?: string;
	initial_params?: UiDuplicateDependentUserItemInitialParam[];
}

export type UiEditIntent =
	| { kind: 'beginEdit'; client_edit_id: string; label?: string }
	| { kind: 'endEdit'; client_edit_id: string }
	| { kind: 'setParam'; node: NodeId; value: ParamValue; behaviour: ParamEventBehaviour }
	| { kind: 'setTextParamSmart'; node: NodeId; value: string; behaviour?: ParamEventBehaviour }
	| { kind: 'setParamControlState'; node: NodeId; state: UiParameterControlState }
	| { kind: 'setParamConstraints'; node: NodeId; constraints: UiParamConstraints }
	| { kind: 'moveNode'; node: NodeId; new_parent: NodeId; new_prev_sibling?: NodeId }
	| { kind: 'removeNode'; node: NodeId }
	| { kind: 'removeNodes'; nodes: NodeId[] }
	| {
			kind: 'createUserItem';
			parent: NodeId;
			node_type: string;
			label?: string;
			initial_params?: UiCreateUserItemInitialParam[];
	  }
	| {
			kind: 'createDashboardContainerWidget';
			parent: NodeId;
			label?: string;
			placement?: UiDashboardWidgetPlacement;
			layout_kind?: string;
			prev_sibling?: NodeId;
	  }
	| {
			kind: 'createDashboardNodeWidget';
			parent: NodeId;
			target: NodeId;
			placement?: UiDashboardWidgetPlacement;
			prev_sibling?: NodeId;
	  }
	| {
			kind: 'createDashboardGenericWidget';
			parent: NodeId;
			target: NodeId;
			placement?: UiDashboardWidgetPlacement;
			prev_sibling?: NodeId;
	  }
	| { kind: 'bindDashboardNodeWidgetTarget'; widget: NodeId; target: NodeId }
	| { kind: 'bindDashboardGenericWidgetTarget'; widget: NodeId; target: NodeId }
	| {
			kind: 'wrapDashboardWidgetInContainer';
			widget: NodeId;
			placement?: UiDashboardWidgetPlacement;
			layout_kind?: string;
	  }
	| {
			kind: 'duplicateNode';
			source: NodeId;
			new_parent: NodeId;
			new_prev_sibling?: NodeId;
			initial_params?: UiCreateUserItemInitialParam[];
	  }
	| {
			kind: 'duplicateNodes';
			nodes?: UiDuplicateNodeSpec[];
			created_items?: UiDuplicateCreateUserItemSpec[];
			dependent_items?: UiDuplicateDependentUserItem[];
	  }
	| {
			kind: 'fitAnimationCurvePath';
			curve: NodeId;
			points: UiAnimationCurveFitPoint[];
			options: UiAnimationCurveBezierFitOptions;
	  }
	| { kind: 'patchMeta'; node: NodeId; patch: Partial<UiNodeMetaDto> }
	| { kind: 'reevaluateGraph' }
	| { kind: 'clearLogs' }
	| { kind: 'setLogMaxEntries'; max_entries: number }
	| { kind: 'undo' }
	| { kind: 'redo' };

export interface UiAck {
	success: boolean;
	status: UiAckStatus;
	error_code?: string;
	error_message?: string;
	earliest_event_time?: EventTime;
	history?: UiHistoryState;
}

export interface UiProjectLoadProblem {
	stage: string;
	message: string;
}

export interface UiProjectLoadRecovery {
	problems: UiProjectLoadProblem[];
}

export interface UiProjectFileLoadResult {
	path: string;
	ui_state?: unknown;
	recovery?: UiProjectLoadRecovery;
}

export interface UiProjectLoadOptions {
	recover?: boolean;
}

export interface UiClient {
	snapshot(scope?: UiSubscriptionScope): Promise<UiSnapshot>;
	subscribe(
		scope: UiSubscriptionScope,
		from: EventTime | undefined,
		onBatch: (batch: UiEventBatch) => void
	): () => void;
	sendIntent(intent: UiEditIntent): Promise<UiAck>;
	sendIntents(intents: UiEditIntent[]): Promise<UiAck[]>;
	replay(scope: UiSubscriptionScope, from?: EventTime): Promise<UiEventBatch>;
	referenceTargets(paramNodeId: NodeId): Promise<UiReferenceTargets>;
	paramControlInfo(paramNodeId: NodeId): Promise<UiParamControlInfo>;
	scriptState(nodeId: NodeId): Promise<UiScriptState>;
	setScriptConfig(nodeId: NodeId, config: UiScriptConfig, forceReload?: boolean): Promise<void>;
	reloadScript(nodeId: NodeId): Promise<void>;
	projectNew(): Promise<void>;
	projectSave(path: string, uiState?: unknown): Promise<void>;
	projectLoad(path: string, options?: UiProjectLoadOptions): Promise<UiProjectFileLoadResult>;
	projectUploadLoad(
		fileName: string,
		contents: string,
		options?: UiProjectLoadOptions
	): Promise<UiProjectFileLoadResult>;
}

export const wholeGraphScope: UiSubscriptionScope = { kind: 'wholeGraph' };
