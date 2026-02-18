export type NodeId = number;

export type ParamEventBehaviour = 'Coalesce' | 'Append';
export type ParamConstraintPolicy = 'ClampAdapt' | 'Reject';
export type UiAckStatus = 'applied' | 'staged' | 'rejected';

export type ParamValue =
	| { kind: 'trigger' }
	| { kind: 'int'; value: number }
	| { kind: 'float'; value: number }
	| { kind: 'str'; value: string }
	| { kind: 'enum'; value: string }
	| { kind: 'bool'; value: boolean }
	| { kind: 'vec2'; value: [number, number] }
	| { kind: 'vec3'; value: [number, number, number] }
	| { kind: 'color'; value: [number, number, number, number] }
	| { kind: 'reference'; uuid: string; cached_id?: NodeId };

export type UiSubscriptionScope =
	| { kind: 'wholeGraph' }
	| { kind: 'subtree'; root: NodeId; max_depth: number };

export type UiUserNodeRole = 'regular' | 'itemRoot';

export interface EventTime {
	tick: number;
	micro: number;
	seq: number;
}

export interface UiNodeMetaDto {
	short_name: string;
	label: string;
	enabled: boolean;
	can_be_disabled: boolean;
	description?: string;
	tags: string[];
}

export interface ParamEnumOption {
	variant_id: string;
	value: ParamValue;
	label: string;
	tags: string[];
	ordering?: number;
}

export interface UiParamConstraints {
	min?: number;
	max?: number;
	step?: number;
	step_base?: number;
	enum_options: ParamEnumOption[];
	policy: ParamConstraintPolicy;
}

export interface UiParamHints {
	widget?: string;
	unit?: string;
}

export interface UiParamDto {
	value: ParamValue;
	event_behaviour: ParamEventBehaviour;
	read_only: boolean;
	constraints: UiParamConstraints;
	ui_hints: UiParamHints;
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
}

export interface UiSchemaView {
	node_types: Array<{ node_type: string }>;
	enums: Array<{
		enum_id: string;
		variants: Array<{ variant_id: string; label: string; tags: string[]; ordering?: number }>;
	}>;
}

export interface UiHistoryState {
	can_undo: boolean;
	can_redo: boolean;
	undo_len: number;
	redo_len: number;
	active_edit_session: boolean;
}

export interface UiSnapshot {
	protocol_version: string;
	scope: UiSubscriptionScope;
	at: EventTime;
	nodes: UiNodeDto[];
	schema: UiSchemaView;
	history: UiHistoryState;
}

export type UiEventKind =
	| { kind: 'paramChanged'; param: NodeId; old_value: ParamValue; new_value: ParamValue }
	| { kind: 'childAdded'; parent: NodeId; child: NodeId; decl_id: string }
	| { kind: 'childRemoved'; parent: NodeId; child: NodeId }
	| { kind: 'childReplaced'; parent: NodeId; old: NodeId; new: NodeId; decl_id: string }
	| { kind: 'childMoved'; child: NodeId; old_parent: NodeId; new_parent: NodeId }
	| { kind: 'childReordered'; parent: NodeId; child: NodeId }
	| { kind: 'nodeCreated'; node: NodeId }
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
	events: UiEventDto[];
}

export type UiEditIntent =
	| { kind: 'beginEdit'; client_edit_id: string; label?: string }
	| { kind: 'endEdit'; client_edit_id: string }
	| { kind: 'setParam'; node: NodeId; value: ParamValue; behaviour: ParamEventBehaviour }
	| { kind: 'moveNode'; node: NodeId; new_parent: NodeId; new_prev_sibling?: NodeId }
	| { kind: 'removeNode'; node: NodeId }
	| { kind: 'createUserItem'; parent: NodeId; node_type: string; label?: string }
	| { kind: 'patchMeta'; node: NodeId; patch: Partial<UiNodeMetaDto> }
	| { kind: 'reevaluateGraph' }
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

export interface UiClient {
	snapshot(scope?: UiSubscriptionScope): Promise<UiSnapshot>;
	subscribe(
		scope: UiSubscriptionScope,
		from: EventTime | undefined,
		onBatch: (batch: UiEventBatch) => void
	): () => void;
	sendIntent(intent: UiEditIntent): Promise<UiAck>;
	replay(scope: UiSubscriptionScope, from?: EventTime): Promise<UiEventBatch>;
}

export const wholeGraphScope: UiSubscriptionScope = { kind: 'wholeGraph' };
