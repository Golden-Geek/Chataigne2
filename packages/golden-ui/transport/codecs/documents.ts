import type {
	UiAck,
	UiCreatableUserItem,
	UiCreateUserItemInitialParam,
	UiDuplicateDependentUserItemInitialParam,
	UiEditIntent,
	UiEventBatch,
	UiEventDto,
	UiNodeDataDto,
	UiNodeDto,
	UiParamConstraints,
	UiParamDto,
	UiSnapshot
} from '../../types';
import type { UiAck as RustUiAck } from '../../generated/rust_protocol/UiAck';
import type { UiDeclaredDescriptionDescriptor as RustUiDeclaredDescriptionDescriptor } from '../../generated/rust_protocol/UiDeclaredDescriptionDescriptor';
import type { UiEditIntent as RustUiEditIntent } from '../../generated/rust_protocol/UiEditIntent';
import type { UiEventBatch as RustUiEventBatch } from '../../generated/rust_protocol/UiEventBatch';
import type { UiEnumDefinition as RustUiSchemaEnumDefinition } from '../../generated/rust_protocol/UiEnumDefinition';
import type { UiEnumVariantDefinition as RustUiSchemaEnumVariantDefinition } from '../../generated/rust_protocol/UiEnumVariantDefinition';
import type { UiNodeDto as RustUiNodeDto } from '../../generated/rust_protocol/UiNodeDto';
import type { UiNodeTypeDescriptor as RustUiNodeTypeDescriptor } from '../../generated/rust_protocol/UiNodeTypeDescriptor';
import type { UiParamDto as RustUiParamDto } from '../../generated/rust_protocol/UiParamDto';
import type { UiSnapshot as RustUiSnapshot } from '../../generated/rust_protocol/UiSnapshot';
import {
	fromRustParamValue,
	fromRustConstraints,
	fromRustControlState,
	isRecord,
	fromRustScope,
	toRustParamValue,
	toRustConstraints
} from './values';
import { fromRustProjectFileSpec } from './projects';

type RustUiEventDto = RustUiEventBatch['events'][number];
type RustCreatableUserItem = NonNullable<RustUiNodeDto['creatable_user_items']>[number];

type EnumOptionsById = Map<string, UiParamConstraints['enum_options']>;

const fromRustSchemaEnumVariants = (
	variants: RustUiSchemaEnumVariantDefinition[] | undefined
): UiSnapshot['schema']['enums'][number]['variants'] => {
	const parsed: UiSnapshot['schema']['enums'][number]['variants'] = [];
	for (const variant of variants ?? []) {
		const variantId = typeof variant.variant_id === 'string' ? variant.variant_id : '';
		if (variantId.length === 0) {
			continue;
		}
		const value =
			variant.value !== undefined
				? fromRustParamValue(variant.value)
				: ({ kind: 'enum', value: variantId } as const);
		parsed.push({
			variant_id: variantId,
			value,
			label: typeof variant.label === 'string' ? variant.label : variantId,
			tags: Array.isArray(variant.tags)
				? variant.tags.filter((tag): tag is string => typeof tag === 'string')
				: [],
			ordering:
				typeof variant.ordering === 'number' && Number.isFinite(variant.ordering)
					? variant.ordering
					: undefined
		});
	}
	return parsed;
};

const parseRustSchemaEnums = (
	definitions: RustUiSchemaEnumDefinition[] | undefined
): {
	enums: UiSnapshot['schema']['enums'];
	enumOptionsById: EnumOptionsById;
} => {
	const enums: UiSnapshot['schema']['enums'] = [];
	const enumOptionsById: EnumOptionsById = new Map();
	for (const definition of definitions ?? []) {
		const enumId = typeof definition.enum_id === 'string' ? definition.enum_id : '';
		if (enumId.length === 0) {
			continue;
		}
		const variants = fromRustSchemaEnumVariants(definition.variants);
		if (variants.length === 0) {
			continue;
		}
		enums.push({ enum_id: enumId, variants });
		enumOptionsById.set(
			enumId,
			variants.map((variant) => ({
				variant_id: variant.variant_id,
				value: variant.value,
				label: variant.label,
				tags: [...variant.tags],
				ordering: variant.ordering
			}))
		);
	}
	return { enums, enumOptionsById };
};

const parseRustNodeTypeDescriptors = (
	descriptors: RustUiNodeTypeDescriptor[] | undefined
): UiSnapshot['schema']['node_types'] => {
	const parsed: UiSnapshot['schema']['node_types'] = [];
	for (const descriptor of descriptors ?? []) {
		const nodeType = typeof descriptor.node_type === 'string' ? descriptor.node_type.trim() : '';
		if (nodeType.length === 0) {
			continue;
		}
		const description =
			typeof descriptor.description === 'string' && descriptor.description.trim().length > 0
				? descriptor.description
				: undefined;
		parsed.push({
			node_type: nodeType,
			description
		});
	}
	return parsed;
};

const parseRustDeclaredDescriptions = (
	descriptors: RustUiDeclaredDescriptionDescriptor[] | undefined
): UiSnapshot['schema']['declared_descriptions'] => {
	const parsed: UiSnapshot['schema']['declared_descriptions'] = [];
	for (const descriptor of descriptors ?? []) {
		const key = typeof descriptor.key === 'string' ? descriptor.key.trim() : '';
		const description =
			typeof descriptor.description === 'string' ? descriptor.description.trim() : '';
		if (key.length === 0 || description.length === 0) {
			continue;
		}
		parsed.push({ key, description });
	}
	return parsed;
};

const fromRustParam = (param: RustUiParamDto, enumOptionsById: EnumOptionsById): UiParamDto => {
	const sharedEnumOptions =
		typeof param.enum_options_id === 'string'
			? enumOptionsById.get(param.enum_options_id)
			: undefined;
	const value = fromRustParamValue(param.value);
	return {
		value,
		default_value:
			param.default_value !== undefined ? fromRustParamValue(param.default_value) : value,
		event_behaviour: param.event_behaviour === 'Append' ? 'Append' : 'Coalesce',
		read_only: Boolean(param.read_only),
		constraints: fromRustConstraints(param.constraints, sharedEnumOptions),
		ui_hints: {
			widget: param.ui_hints?.widget ?? undefined,
			unit: param.ui_hints?.unit ?? undefined
		},
		control: fromRustControlState(param.control),
		reference_allowed_targets: [...(param.reference_allowed_targets ?? [])],
		reference_visible_nodes: [...(param.reference_visible_nodes ?? [])]
	};
};

const fromRustNodeData = (
	data: RustUiNodeDto['data'],
	enumOptionsById: EnumOptionsById
): UiNodeDataDto => {
	if (data.kind === 'parameter') {
		return {
			kind: 'parameter',
			param: fromRustParam(data.param, enumOptionsById)
		};
	}
	return { kind: 'node', node_type: data.node_type };
};

const fromRustCreatableUserItem = (item: RustCreatableUserItem): UiCreatableUserItem => ({
	node_type: item.node_type,
	item_kind: item.item_kind,
	label: item.label,
	menu_path: Array.isArray(item.menu_path)
		? item.menu_path
				.map((segment) => (typeof segment === 'string' ? segment.trim() : ''))
				.filter((segment) => segment.length > 0)
		: [],
	initial_params: Array.isArray(item.initial_params)
		? item.initial_params.map((entry) => ({
				decl_id: entry.decl_id,
				value: fromRustParamValue(entry.value)
			}))
		: [],
	icon: item.icon ?? undefined,
	select_when_created: Boolean(item.select_when_created),
	separator_before: Boolean(item.separator_before)
});

const fromRustNode = (node: RustUiNodeDto, enumOptionsById: EnumOptionsById): UiNodeDto => ({
	node_id: node.node_id,
	uuid: node.uuid,
	decl_id: node.decl_id,
	node_type: node.node_type,
	meta: {
		short_name: node.meta.short_name,
		label: node.meta.label,
		enabled: Boolean(node.meta.enabled),
		can_be_disabled: Boolean(node.meta.can_be_disabled),
		user_permissions: {
			can_edit_name: Boolean(node.meta.user_permissions?.can_edit_name),
			can_remove_and_duplicate: Boolean(node.meta.user_permissions?.can_remove_and_duplicate),
			can_edit_constraints: Boolean(node.meta.user_permissions?.can_edit_constraints),
			can_edit_tags: Boolean(node.meta.user_permissions?.can_edit_tags),
			can_edit_color: Boolean(node.meta.user_permissions?.can_edit_color)
		},
		description: typeof node.meta.description === 'string' ? node.meta.description : undefined,
		declared_description_key:
			typeof node.meta.declared_description_key === 'string' &&
			node.meta.declared_description_key.trim().length > 0
				? node.meta.declared_description_key
				: undefined,
		description_overridden: Boolean(node.meta.description_overridden),
		tags: [...(node.meta.tags ?? [])],
		presentation: node.meta.presentation
			? {
					color: node.meta.presentation.color
						? {
								r: node.meta.presentation.color.r,
								g: node.meta.presentation.color.g,
								b: node.meta.presentation.color.b,
								a: node.meta.presentation.color.a
							}
						: undefined,
					default_color: node.meta.presentation.default_color
						? {
								r: node.meta.presentation.default_color.r,
								g: node.meta.presentation.default_color.g,
								b: node.meta.presentation.default_color.b,
								a: node.meta.presentation.default_color.a
							}
						: undefined,
					icon: node.meta.presentation.icon ?? undefined,
					collapsed: node.meta.presentation.collapsed ?? undefined,
					warnings: Array.isArray(node.meta.presentation.warnings)
						? node.meta.presentation.warnings.map((warning) => ({
								id: warning.id ?? undefined,
								message: warning.message,
								detail: warning.detail ?? undefined
							}))
						: undefined,
					show_child_warnings_max_depth:
						node.meta.presentation.show_child_warnings_max_depth ?? undefined,
					show_in_nested_inspector: node.meta.presentation.show_in_nested_inspector ?? undefined,
					show_in_inspector_content: node.meta.presentation.show_in_inspector_content ?? undefined
				}
			: undefined
	},
	data: fromRustNodeData(node.data, enumOptionsById),
	user_role: node.user_role ?? 'regular',
	user_item_kind: node.user_item_kind ?? node.node_type,
	accepted_user_item_kinds: [...(node.accepted_user_item_kinds ?? [])],
	creatable_user_items: (node.creatable_user_items ?? []).map(fromRustCreatableUserItem),
	children: [...(node.children ?? [])]
});

const fromRustEventNodeSnapshot = (value: unknown): UiNodeDto | undefined => {
	if (!isRecord(value)) {
		return undefined;
	}
	return fromRustNode(value as RustUiNodeDto, new Map());
};

const fromRustEvent = (event: RustUiEventDto): UiEventDto => {
	if (!isRecord(event)) {
		return event as UiEventDto;
	}

	const nestedKind = event.kind;
	if (isRecord(nestedKind) && typeof nestedKind.kind === 'string') {
		if (nestedKind.kind === 'paramChanged') {
			return {
				...(event as Omit<UiEventDto, 'kind'>),
				kind: {
					kind: 'paramChanged',
					param: Number(nestedKind.param ?? 0),
					old_value: fromRustParamValue(nestedKind.old_value),
					new_value: fromRustParamValue(nestedKind.new_value)
				}
			};
		}
		if (nestedKind.kind === 'paramControlChanged') {
			return {
				...(event as Omit<UiEventDto, 'kind'>),
				kind: {
					kind: 'paramControlChanged',
					param: Number(nestedKind.param ?? 0),
					old_state: fromRustControlState(nestedKind.old_state),
					new_state: fromRustControlState(nestedKind.new_state)
				}
			};
		}
		if (nestedKind.kind === 'paramConstraintsChanged') {
			return {
				...(event as Omit<UiEventDto, 'kind'>),
				kind: {
					kind: 'paramConstraintsChanged',
					param: Number(nestedKind.param ?? 0),
					old_constraints: fromRustConstraints(
						nestedKind.old_constraints as RustUiParamDto['constraints'],
						undefined
					),
					new_constraints: fromRustConstraints(
						nestedKind.new_constraints as RustUiParamDto['constraints'],
						undefined
					)
				}
			};
		}
		if (nestedKind.kind === 'nodeCreated') {
			return {
				...(event as Omit<UiEventDto, 'kind'>),
				kind: {
					kind: 'nodeCreated',
					node: Number(nestedKind.node ?? 0),
					snapshot: fromRustEventNodeSnapshot(nestedKind.snapshot)
				}
			};
		}
		return event as unknown as UiEventDto;
	}

	if (typeof nestedKind !== 'string') {
		return event as unknown as UiEventDto;
	}

	if (nestedKind === 'paramChanged') {
		return {
			...(event as Omit<UiEventDto, 'kind'>),
			kind: {
				kind: 'paramChanged',
				param: Number((event as Record<string, unknown>).param ?? 0),
				old_value: fromRustParamValue((event as Record<string, unknown>).old_value),
				new_value: fromRustParamValue((event as Record<string, unknown>).new_value)
			}
		};
	}
	if (nestedKind === 'paramControlChanged') {
		return {
			...(event as Omit<UiEventDto, 'kind'>),
			kind: {
				kind: 'paramControlChanged',
				param: Number((event as Record<string, unknown>).param ?? 0),
				old_state: fromRustControlState((event as Record<string, unknown>).old_state),
				new_state: fromRustControlState((event as Record<string, unknown>).new_state)
			}
		};
	}
	if (nestedKind === 'paramConstraintsChanged') {
		return {
			...(event as Omit<UiEventDto, 'kind'>),
			kind: {
				kind: 'paramConstraintsChanged',
				param: Number((event as Record<string, unknown>).param ?? 0),
				old_constraints: fromRustConstraints(
					(event as Record<string, unknown>).old_constraints as RustUiParamDto['constraints'],
					undefined
				),
				new_constraints: fromRustConstraints(
					(event as Record<string, unknown>).new_constraints as RustUiParamDto['constraints'],
					undefined
				)
			}
		};
	}
	if (nestedKind === 'nodeCreated') {
		const payload = event as Record<string, unknown>;
		return {
			...(event as Omit<UiEventDto, 'kind'>),
			kind: {
				kind: 'nodeCreated',
				node: Number(payload.node ?? 0),
				snapshot: fromRustEventNodeSnapshot(payload.snapshot)
			}
		};
	}

	if (nestedKind === 'graphTransaction') {
		const payload = event as Record<string, unknown>;
		const rawOps = Array.isArray(payload.ops) ? payload.ops : [];
		const ops = rawOps.map((op: unknown) => {
			if (isRecord(op) && op.kind === 'nodeCreated') {
				return { ...op, snapshot: fromRustEventNodeSnapshot(op.snapshot) };
			}
			if (isRecord(op) && op.kind === 'subtreeInserted') {
				const rawNodes = Array.isArray(op.nodes) ? (op.nodes as unknown[]) : [];
				return {
					...op,
					nodes: rawNodes.map((n) => fromRustEventNodeSnapshot(n))
				};
			}
			if (isRecord(op) && op.kind === 'paramPatched' && isRecord(op.patch)) {
				return {
					...op,
					patch: {
						...(op.patch.value === undefined ? {} : { value: fromRustParamValue(op.patch.value) }),
						...(op.patch.control === undefined
							? {}
							: { control: fromRustControlState(op.patch.control) }),
						...(op.patch.constraints === undefined
							? {}
							: {
									constraints: fromRustConstraints(
										op.patch.constraints as RustUiParamDto['constraints'],
										undefined
									)
								})
					}
				};
			}
			return op;
		});
		return {
			...(event as Omit<UiEventDto, 'kind'>),
			kind: {
				kind: 'graphTransaction',
				tx_id: Number(payload.tx_id ?? 0),
				epoch: Number(payload.epoch ?? 0),
				base_graph_version: Number(payload.base_graph_version ?? 0),
				next_graph_version: Number(payload.next_graph_version ?? 0),
				ops
			} as UiEventDto['kind']
		};
	}

	const payload = { ...(event as Record<string, unknown>) };
	delete payload.time;
	delete payload.kind;
	return {
		...(event as Omit<UiEventDto, 'kind'>),
		kind: {
			kind: nestedKind,
			...(payload as Record<string, unknown>)
		} as UiEventDto['kind']
	};
};

export const fromRustSnapshot = (snapshot: RustUiSnapshot): UiSnapshot => {
	const parsedSchemaEnums = parseRustSchemaEnums(snapshot.schema.enums);
	return {
		protocol_version: snapshot.protocol_version,
		scope: fromRustScope(snapshot.scope),
		at: snapshot.at,
		nodes: snapshot.nodes.map((node) => fromRustNode(node, parsedSchemaEnums.enumOptionsById)),
		schema: {
			node_types: parseRustNodeTypeDescriptors(snapshot.schema.node_types),
			declared_descriptions: parseRustDeclaredDescriptions(snapshot.schema.declared_descriptions),
			enums: parsedSchemaEnums.enums
		},
		history: snapshot.history ?? {
			can_undo: false,
			can_redo: false,
			undo_len: 0,
			redo_len: 0,
			active_edit_session: false,
			current_history_state_id: 0
		},
		logger: {
			max_entries: snapshot.logger?.max_entries ?? 0,
			records: (snapshot.logger?.records ?? []).map((record) => ({
				id: record.id,
				timestamp_ms: record.timestamp_ms,
				level: record.level,
				tag: record.tag,
				message: record.message,
				repeat_count: record.repeat_count ?? undefined,
				origin: record.origin ?? undefined
			}))
		},
		project_file: fromRustProjectFileSpec(snapshot.project_file)
	};
};

export const fromRustEventBatch = (batch: RustUiEventBatch): UiEventBatch => ({
	from: batch.from ?? undefined,
	to: batch.to ?? undefined,
	runtime: batch.runtime ?? undefined,
	events: batch.events.map(fromRustEvent)
});

export const fromRustAck = (ack: RustUiAck): UiAck => ({
	success: ack.success,
	status: ack.status,
	error_code: ack.error_code ?? undefined,
	error_message: ack.error_message ?? undefined,
	earliest_event_time: ack.earliest_event_time ?? undefined,
	latest_event_time: ack.latest_event_time ?? undefined,
	history: ack.history
});

const toRustInitialParams = (initialParams: UiCreateUserItemInitialParam[] | undefined) =>
	initialParams?.map((entry) => ({
		...entry,
		value: toRustParamValue(entry.value)
	}));

const toRustDuplicateDependentInitialParams = (
	initialParams: UiDuplicateDependentUserItemInitialParam[] | undefined
) =>
	initialParams?.map((entry) => ({
		...entry,
		value:
			entry.value.kind === 'literal'
				? { ...entry.value, value: toRustParamValue(entry.value.value) }
				: entry.value
	}));

export const toRustIntent = (intent: UiEditIntent): RustUiEditIntent => {
	if (intent.kind === 'setParam') {
		return {
			...intent,
			value: toRustParamValue(intent.value)
		} as RustUiEditIntent;
	}
	if (intent.kind === 'createUserItem' && intent.initial_params) {
		return {
			...intent,
			initial_params: toRustInitialParams(intent.initial_params)
		} as RustUiEditIntent;
	}
	if (intent.kind === 'duplicateNode' && intent.initial_params) {
		return {
			...intent,
			initial_params: toRustInitialParams(intent.initial_params)
		} as RustUiEditIntent;
	}
	if (intent.kind === 'duplicateNodes') {
		return {
			...intent,
			nodes: intent.nodes?.map((entry) => ({
				...entry,
				initial_params: toRustInitialParams(entry.initial_params)
			})),
			created_items: intent.created_items?.map((entry) => ({
				...entry,
				initial_params: toRustInitialParams(entry.initial_params)
			})),
			dependent_items: intent.dependent_items?.map((entry) => ({
				...entry,
				initial_params: toRustDuplicateDependentInitialParams(entry.initial_params)
			}))
		} as RustUiEditIntent;
	}
	if (intent.kind === 'setParamControlState') {
		return {
			...intent,
			state: {
				mode: intent.state.mode,
				spec: intent.state.spec
			}
		} as RustUiEditIntent;
	}
	if (intent.kind === 'setParamConstraints') {
		return {
			...intent,
			constraints: toRustConstraints(intent.constraints)
		} as RustUiEditIntent;
	}
	return intent as RustUiEditIntent;
};
