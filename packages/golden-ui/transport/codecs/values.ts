import type {
	ParamValue,
	UiParamControlInfo,
	UiParameterControlMode,
	UiParameterControlSpec,
	UiParameterControlState,
	UiParamConstraints,
	UiFileConstraints,
	UiReferenceConstraints,
	UiReferenceTargets,
	UiReferenceRoot,
	UiSubscriptionScope,
	UiTokenSuggestion,
	UiUserContextCandidate,
	UiUserContextValueType,
	UiParamControlCandidate,
	UiParamValueProjection
} from '../../types';
import type { ParamValue as RustParamValue } from '../../generated/rust_protocol/ParamValue';
import type { UiParamControlInfoDto as RustUiParamControlInfoResponse } from '../../generated/rust_protocol/UiParamControlInfoDto';
import type { UiParamDto as RustUiParamDto } from '../../generated/rust_protocol/UiParamDto';
import type { UiReferenceTargetsDto as RustReferenceTargetsResponse } from '../../generated/rust_protocol/UiReferenceTargetsDto';
import type { UiSubscriptionScope as RustUiSubscriptionScope } from '../../generated/rust_protocol/UiSubscriptionScope';
import { isCssUnit } from '../../css-value';

export type RustScope = RustUiSubscriptionScope;

export const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

export const toRustScope = (scope: UiSubscriptionScope): RustScope => {
	if (scope.kind === 'wholeGraph') {
		return 'wholeGraph';
	}
	return { subtree: { root: scope.root, max_depth: scope.max_depth } };
};

export const fromRustScope = (scope: unknown): UiSubscriptionScope => {
	if (scope === 'wholeGraph' || scope === 'WholeGraph') {
		return { kind: 'wholeGraph' };
	}

	if (isRecord(scope)) {
		if (scope.kind === 'wholeGraph') {
			return { kind: 'wholeGraph' };
		}
		if (scope.kind === 'subtree') {
			const root = Number(scope.root ?? 0);
			const maxDepth = Number(scope.max_depth ?? 0);
			return { kind: 'subtree', root, max_depth: maxDepth };
		}

		const subtree = scope.subtree;
		if (isRecord(subtree)) {
			return {
				kind: 'subtree',
				root: Number(subtree.root ?? 0),
				max_depth: Number(subtree.max_depth ?? 0)
			};
		}
	}

	return { kind: 'wholeGraph' };
};

const fromRustReferencePayload = (
	payload: unknown
): {
	uuid: string;
	projection?: unknown;
	cached_id?: number;
	cached_name?: string;
	relative_path_from_root?: string[];
} => {
	if (typeof payload === 'string') {
		return { uuid: payload };
	}

	if (isRecord(payload)) {
		const rawUuid = payload.uuid;
		const uuid =
			typeof rawUuid === 'string'
				? rawUuid
				: isRecord(rawUuid) && typeof rawUuid['0'] === 'string'
					? String(rawUuid['0'])
					: '';

		const cached = payload.cached_id;
		const cached_id = typeof cached === 'number' ? cached : undefined;
		const cachedName = payload.cached_name;
		const cached_name = typeof cachedName === 'string' ? cachedName : undefined;
		const relativePathRaw = payload.relative_path_from_root;
		const relative_path_from_root = Array.isArray(relativePathRaw)
			? relativePathRaw.filter((segment): segment is string => typeof segment === 'string')
			: undefined;
		return {
			uuid,
			projection: payload.projection,
			cached_id,
			cached_name,
			relative_path_from_root
		};
	}

	return { uuid: '' };
};

export const fromRustParamValue = (value: unknown): ParamValue => {
	if (typeof value === 'string') {
		if (value === 'Trigger' || value === 'trigger') {
			return { kind: 'trigger' };
		}
		if (value === 'BoolTrue') {
			return { kind: 'bool', value: true };
		}
		if (value === 'BoolFalse') {
			return { kind: 'bool', value: false };
		}
		return { kind: 'str', value };
	}

	if (!isRecord(value)) {
		return { kind: 'str', value: String(value) };
	}

	if ('Int' in value) {
		return { kind: 'int', value: Number(value.Int ?? 0) };
	}
	if ('Float' in value) {
		return { kind: 'float', value: Number(value.Float ?? 0) };
	}
	if ('Str' in value) {
		return { kind: 'str', value: String(value.Str ?? '') };
	}
	if ('Bytes' in value && Array.isArray(value.Bytes)) {
		return { kind: 'bytes', value: value.Bytes as number[] };
	}
	if ('File' in value) {
		return { kind: 'file', value: String(value.File ?? '') };
	}
	if ('Enum' in value) {
		return { kind: 'enum', value: String(value.Enum ?? '') };
	}
	if ('Bool' in value) {
		return { kind: 'bool', value: Boolean(value.Bool) };
	}
	if ('CssValue' in value && isRecord(value.CssValue)) {
		const cssValue = value.CssValue;
		const unit = isCssUnit(cssValue.unit) ? cssValue.unit : 'rem';
		return {
			kind: 'css_value',
			value: Number(cssValue.value ?? 0),
			unit
		};
	}
	if ('Vec2' in value && Array.isArray(value.Vec2)) {
		const vec = value.Vec2 as unknown[];
		return { kind: 'vec2', value: [Number(vec[0] ?? 0), Number(vec[1] ?? 0)] };
	}
	if ('Vec3' in value && Array.isArray(value.Vec3)) {
		const vec = value.Vec3 as unknown[];
		return {
			kind: 'vec3',
			value: [Number(vec[0] ?? 0), Number(vec[1] ?? 0), Number(vec[2] ?? 0)]
		};
	}
	if ('Color' in value && Array.isArray(value.Color)) {
		const vec = value.Color as unknown[];
		return {
			kind: 'color',
			value: [Number(vec[0] ?? 0), Number(vec[1] ?? 0), Number(vec[2] ?? 0), Number(vec[3] ?? 1)]
		};
	}
	if ('Reference' in value) {
		const reference = fromRustReferencePayload(value.Reference);
		return {
			kind: 'reference',
			uuid: reference.uuid,
			projection: isUiParamValueProjection(reference.projection) ? reference.projection : undefined,
			cached_id: reference.cached_id,
			cached_name: reference.cached_name,
			relative_path_from_root: reference.relative_path_from_root
		};
	}
	if ('Trigger' in value) {
		return { kind: 'trigger' };
	}

	return { kind: 'str', value: JSON.stringify(value) };
};

export const toRustParamValue = (value: ParamValue): RustParamValue => {
	switch (value.kind) {
		case 'trigger':
			return { Trigger: [] };
		case 'int':
			return { Int: value.value };
		case 'float':
			return { Float: value.value };
		case 'str':
			return { Str: value.value };
		case 'bytes':
			return { Bytes: value.value };
		case 'file':
			return { File: value.value };
		case 'enum':
			return { Enum: value.value };
		case 'bool':
			return { Bool: value.value };
		case 'css_value':
			return { CssValue: { value: value.value, unit: value.unit } };
		case 'vec2':
			return { Vec2: value.value };
		case 'vec3':
			return { Vec3: value.value };
		case 'color':
			return { Color: value.value };
		case 'reference':
			return {
				Reference: {
					uuid: value.uuid,
					projection: value.projection,
					cached_name: value.cached_name,
					relative_path_from_root: value.relative_path_from_root ?? []
				}
			};
	}
};

const fromRustReferenceRoot = (root: unknown): UiReferenceRoot => {
	if (typeof root === 'string') {
		if (root === 'EngineRoot' || root === 'engineRoot') {
			return { kind: 'engineRoot' };
		}
	}

	if (isRecord(root)) {
		if ('Uuid' in root) {
			const uuid = root.Uuid;
			return { kind: 'uuid', uuid: typeof uuid === 'string' ? uuid : '' };
		}
		if ('RelativeToOwner' in root) {
			const payload = root.RelativeToOwner;
			const path =
				isRecord(payload) && Array.isArray(payload.path)
					? payload.path.filter((segment): segment is string => typeof segment === 'string')
					: [];
			return { kind: 'relativeToOwner', path };
		}
		if (root.kind === 'uuid') {
			return { kind: 'uuid', uuid: String(root.uuid ?? '') };
		}
		if (root.kind === 'relativeToOwner') {
			const rawPath = root.path;
			const path = Array.isArray(rawPath)
				? rawPath.filter((segment): segment is string => typeof segment === 'string')
				: [];
			return { kind: 'relativeToOwner', path };
		}
	}

	return { kind: 'engineRoot' };
};

const fromRustReferenceConstraints = (reference: unknown): UiReferenceConstraints | undefined => {
	if (!isRecord(reference)) {
		return undefined;
	}

	const allowedNodeTypes = Array.isArray(reference.allowed_node_types)
		? reference.allowed_node_types.filter((value): value is string => typeof value === 'string')
		: [];
	const allowedParameterTypes = Array.isArray(reference.allowed_parameter_types)
		? reference.allowed_parameter_types.filter(
				(value): value is string => typeof value === 'string'
			)
		: [];

	const target_kind =
		reference.target_kind === 'ParameterOnly' || reference.target_kind === 'parameterOnly'
			? 'parameterOnly'
			: 'anyNode';

	return {
		root: fromRustReferenceRoot(reference.root),
		target_kind,
		allowed_node_types: allowedNodeTypes,
		allowed_parameter_types: allowedParameterTypes,
		allow_projections:
			typeof reference.allow_projections === 'boolean' ? reference.allow_projections : true,
		custom_filter_key:
			typeof reference.custom_filter_key === 'string' ? reference.custom_filter_key : undefined,
		default_search_filter:
			typeof reference.default_search_filter === 'string'
				? reference.default_search_filter
				: undefined
	};
};

const fromRustFileConstraints = (file: unknown): UiFileConstraints | undefined => {
	if (!isRecord(file)) {
		return undefined;
	}

	const allowedTypesRaw = Array.isArray(file.allowed_types) ? file.allowed_types : [];
	const allowedExtensionsRaw = Array.isArray(file.allowed_extensions)
		? file.allowed_extensions
		: [];
	const allowed_types = allowedTypesRaw
		.map((value: unknown) => String(value).toLowerCase())
		.filter(
			(value): value is UiFileConstraints['allowed_types'][number] =>
				value === 'audio' || value === 'video' || value === 'script'
		);
	const allowed_extensions = allowedExtensionsRaw
		.map((value: unknown) => String(value).trim())
		.filter((value) => value.length > 0);

	return {
		allowed_types,
		allowed_extensions
	};
};

const fromRustRangeConstraint = (range: unknown): UiParamConstraints['range'] | undefined => {
	if (!range || typeof range !== 'object') {
		return undefined;
	}

	const payload = range as Record<string, unknown>;
	const kind = String(payload.kind ?? '').toLowerCase();
	if (kind === 'uniform') {
		return {
			kind: 'uniform',
			min: typeof payload.min === 'number' ? payload.min : undefined,
			max: typeof payload.max === 'number' ? payload.max : undefined
		};
	}

	if (kind === 'components') {
		const parseList = (value: unknown): number[] | undefined => {
			if (!Array.isArray(value)) {
				return undefined;
			}
			const parsed = value.map((entry) => Number(entry)).filter((entry) => Number.isFinite(entry));
			return parsed.length > 0 ? parsed : undefined;
		};

		return {
			kind: 'components',
			min: parseList(payload.min),
			max: parseList(payload.max)
		};
	}

	return undefined;
};

export const fromRustConstraints = (
	constraints: RustUiParamDto['constraints'] | undefined,
	sharedEnumOptions: UiParamConstraints['enum_options'] | undefined
): UiParamConstraints => {
	const payload = (constraints ?? {}) as NonNullable<RustUiParamDto['constraints']>;
	const step =
		typeof payload.step === 'number' && Number.isFinite(payload.step) ? payload.step : undefined;
	const stepBase =
		typeof payload.step_base === 'number' && Number.isFinite(payload.step_base)
			? payload.step_base
			: undefined;
	const inlineEnumOptions = (payload.enum_options ?? []).map((option) => ({
		variant_id: option.variant_id,
		value: fromRustParamValue(option.value),
		label: option.label,
		tags: [...(option.tags ?? [])],
		ordering: option.ordering ?? undefined
	}));
	const enumOptions =
		inlineEnumOptions.length > 0 ? inlineEnumOptions : [...(sharedEnumOptions ?? [])];
	return {
		range: fromRustRangeConstraint(payload.range),
		step,
		step_base: stepBase,
		enum_options: enumOptions,
		policy: payload.policy === 'Reject' ? 'Reject' : 'ClampAdapt',
		reference: fromRustReferenceConstraints(payload.reference),
		file: fromRustFileConstraints(payload.file)
	};
};

const toRustReferenceRoot = (
	root: UiReferenceRoot
): NonNullable<NonNullable<RustUiParamDto['constraints']>['reference']>['root'] => {
	switch (root.kind) {
		case 'uuid':
			return { Uuid: root.uuid };
		case 'relativeToOwner':
			return { RelativeToOwner: { path: [...root.path] } };
		case 'engineRoot':
		default:
			return 'EngineRoot';
	}
};

export const toRustConstraints = (
	constraints: UiParamConstraints
): NonNullable<RustUiParamDto['constraints']> => ({
	range: constraints.range
		? constraints.range.kind === 'uniform'
			? {
					kind: 'uniform',
					min: constraints.range.min,
					max: constraints.range.max
				}
			: {
					kind: 'components',
					min: constraints.range.min ? [...constraints.range.min] : undefined,
					max: constraints.range.max ? [...constraints.range.max] : undefined
				}
		: undefined,
	step: constraints.step,
	step_base: constraints.step_base,
	enum_options: constraints.enum_options.map((option) => ({
		variant_id: option.variant_id,
		value: toRustParamValue(option.value),
		label: option.label,
		tags: [...option.tags],
		ordering: option.ordering
	})),
	policy: constraints.policy,
	reference: constraints.reference
		? {
				root: toRustReferenceRoot(constraints.reference.root),
				target_kind:
					constraints.reference.target_kind === 'parameterOnly' ? 'ParameterOnly' : 'AnyNode',
				allowed_node_types: [...constraints.reference.allowed_node_types],
				allowed_parameter_types: [...constraints.reference.allowed_parameter_types],
				allow_projections: constraints.reference.allow_projections,
				custom_filter_key: constraints.reference.custom_filter_key,
				default_search_filter: constraints.reference.default_search_filter
			}
		: undefined,
	file: constraints.file
		? {
				allowed_types: [...constraints.file.allowed_types],
				allowed_extensions: [...constraints.file.allowed_extensions]
			}
		: undefined
});

const isUiParameterControlMode = (value: unknown): value is UiParameterControlMode =>
	value === 'manual' ||
	value === 'contextLink' ||
	value === 'templateText' ||
	value === 'expression' ||
	value === 'proxy' ||
	value === 'binding' ||
	value === 'animation';

const isUiUserContextValueType = (value: unknown): value is UiUserContextValueType =>
	value === 'trigger' ||
	value === 'int' ||
	value === 'float' ||
	value === 'str' ||
	value === 'file' ||
	value === 'enum' ||
	value === 'bool' ||
	value === 'vec2' ||
	value === 'vec3' ||
	value === 'color' ||
	value === 'reference';

const isUiParamValueProjection = (value: unknown): value is UiParamValueProjection =>
	value === 'floatToVec2X0' ||
	value === 'floatToVec20Y' ||
	value === 'floatToVec2XX' ||
	value === 'floatToVec3X00' ||
	value === 'floatToVec30Y0' ||
	value === 'floatToVec300Z' ||
	value === 'floatToVec3XXX' ||
	value === 'vec2X' ||
	value === 'vec2Y' ||
	value === 'vec2ToVec3XY0' ||
	value === 'vec2ToVec3X0Y' ||
	value === 'vec2ToColorHs' ||
	value === 'vec3X' ||
	value === 'vec3Y' ||
	value === 'vec3Z' ||
	value === 'vec3ToVec2XY' ||
	value === 'vec3ToVec2XZ' ||
	value === 'vec3ToVec2YZ' ||
	value === 'vec3ToColorRgb' ||
	value === 'vec3ToColorHsv' ||
	value === 'colorR' ||
	value === 'colorG' ||
	value === 'colorB' ||
	value === 'colorA' ||
	value === 'colorToVec3Rgb' ||
	value === 'colorToVec3Hsv' ||
	value === 'colorToVec2Hs';

const fromRustUserContextCandidate = (value: unknown): UiUserContextCandidate | null => {
	if (!isRecord(value) || typeof value.symbol !== 'string') {
		return null;
	}

	const valueType = isUiUserContextValueType(value.value_type) ? value.value_type : 'str';
	const multiplex = isRecord(value.multiplex)
		? {
				multiplex: Number(value.multiplex.multiplex ?? 0),
				list: Number(value.multiplex.list ?? 0),
				axis_id: String(value.multiplex.axis_id ?? ''),
				index_link_symbol: String(value.multiplex.index_link_symbol ?? ''),
				index0_link_symbol: String(value.multiplex.index0_link_symbol ?? ''),
				list_link_symbol: String(value.multiplex.list_link_symbol ?? '')
			}
		: undefined;
	return {
		symbol: value.symbol,
		value_type: valueType,
		kind: value.kind === 'multiplexList' ? 'multiplexList' : 'scalar',
		multiplex,
		scope_owner: Number(value.scope_owner ?? 0),
		lexical_depth: Number(value.lexical_depth ?? 0),
		entry_param: Number(value.entry_param ?? 0),
		compatible: Boolean(value.compatible),
		directly_compatible: Boolean(value.directly_compatible),
		multiplex_index_compatible: Boolean(value.multiplex_index_compatible),
		shadowed: Boolean(value.shadowed),
		projections: Array.isArray(value.projections)
			? value.projections.filter((projection): projection is UiParamValueProjection =>
					isUiParamValueProjection(projection)
				)
			: []
	};
};

const fromRustTokenSuggestion = (value: unknown): UiTokenSuggestion | null => {
	if (!isRecord(value) || typeof value.token !== 'string') {
		return null;
	}
	return { token: value.token };
};

const fromRustParamControlCandidate = (value: unknown): UiParamControlCandidate | null => {
	if (!isRecord(value)) {
		return null;
	}
	return {
		param: Number(value.param ?? 0),
		compatible: Boolean(value.compatible),
		projections: Array.isArray(value.projections)
			? value.projections.filter((projection): projection is UiParamValueProjection =>
					isUiParamValueProjection(projection)
				)
			: []
	};
};

const fromRustReferenceTargetCandidate = (
	value: unknown
): {
	target_id: number;
	direct: boolean;
	projections: UiParamValueProjection[];
} | null => {
	if (!isRecord(value)) {
		return null;
	}
	return {
		target_id: Number(value.target ?? 0),
		direct: Boolean(value.direct),
		projections: Array.isArray(value.projections)
			? value.projections.filter((projection): projection is UiParamValueProjection =>
					isUiParamValueProjection(projection)
				)
			: []
	};
};

const defaultControlSpecForMode = (mode: UiParameterControlMode): UiParameterControlSpec => {
	switch (mode) {
		case 'contextLink':
			return { mode: 'contextLink', symbol: '', projection: undefined };
		case 'templateText':
			return { mode: 'templateText', template: '' };
		case 'expression':
			return { mode: 'expression' };
		case 'proxy':
			return { mode: 'proxy' };
		case 'binding':
			return { mode: 'binding' };
		case 'animation':
			return { mode: 'animation' };
		case 'manual':
		default:
			return { mode: 'manual' };
	}
};

const fromRustControlSpec = (
	mode: UiParameterControlMode,
	controlPayload: unknown
): UiParameterControlSpec => {
	if (!isRecord(controlPayload)) {
		return defaultControlSpecForMode(mode);
	}

	switch (mode) {
		case 'contextLink':
			return {
				mode: 'contextLink',
				symbol: typeof controlPayload.symbol === 'string' ? controlPayload.symbol : '',
				projection: isUiParamValueProjection(controlPayload.projection)
					? controlPayload.projection
					: undefined
			};
		case 'templateText':
			return {
				mode: 'templateText',
				template: typeof controlPayload.template === 'string' ? controlPayload.template : ''
			};
		case 'expression':
			return { mode: 'expression' };
		case 'proxy':
			return { mode: 'proxy' };
		case 'binding':
			return { mode: 'binding' };
		case 'animation':
			return { mode: 'animation' };
		case 'manual':
		default:
			return { mode: 'manual' };
	}
};

export const fromRustControlState = (control: unknown): UiParameterControlState => {
	if (!isRecord(control)) {
		return {
			mode: 'manual',
			spec: { mode: 'manual' }
		};
	}

	const modeRaw = control.mode;
	const mode: UiParameterControlMode = isUiParameterControlMode(modeRaw) ? modeRaw : 'manual';

	return {
		mode,
		spec: fromRustControlSpec(mode, control.spec)
	};
};

export const fromRustReferenceTargets = (
	payload: RustReferenceTargetsResponse
): UiReferenceTargets => ({
	allowed_target_ids: [...(payload.allowed_targets ?? [])],
	visible_node_ids: [...(payload.visible_nodes ?? [])],
	candidates: Array.isArray(payload.candidates)
		? payload.candidates
				.map((entry) => fromRustReferenceTargetCandidate(entry))
				.filter(
					(
						entry
					): entry is {
						target_id: number;
						direct: boolean;
						projections: UiParamValueProjection[];
					} => entry !== null
				)
		: []
});

export const fromRustParamControlInfo = (
	payload: RustUiParamControlInfoResponse
): UiParamControlInfo => {
	const activeMode: UiParameterControlMode = isUiParameterControlMode(payload.active_mode)
		? payload.active_mode
		: 'manual';
	const availableModes = Array.isArray(payload.available_modes)
		? payload.available_modes.filter((mode: unknown): mode is UiParameterControlMode =>
				isUiParameterControlMode(mode)
			)
		: [];
	if (!availableModes.includes('manual')) {
		availableModes.unshift('manual');
	}

	return {
		param: Number(payload.param ?? 0),
		active_mode: activeMode,
		available_modes: availableModes,
		context_candidates: Array.isArray(payload.context_candidates)
			? payload.context_candidates
					.map((candidate: unknown) => fromRustUserContextCandidate(candidate))
					.filter(
						(candidate: UiUserContextCandidate | null): candidate is UiUserContextCandidate =>
							candidate !== null
					)
			: [],
		token_suggestions: Array.isArray(payload.token_suggestions)
			? payload.token_suggestions
					.map((entry: unknown) => fromRustTokenSuggestion(entry))
					.filter((entry: UiTokenSuggestion | null): entry is UiTokenSuggestion => entry !== null)
			: [],
		proxy_candidates: Array.isArray(payload.proxy_candidates)
			? payload.proxy_candidates
					.map((entry: unknown) => fromRustParamControlCandidate(entry))
					.filter(
						(entry: UiParamControlCandidate | null): entry is UiParamControlCandidate =>
							entry !== null
					)
			: [],
		binding_candidates: Array.isArray(payload.binding_candidates)
			? payload.binding_candidates
					.map((entry: unknown) => fromRustParamControlCandidate(entry))
					.filter(
						(entry: UiParamControlCandidate | null): entry is UiParamControlCandidate =>
							entry !== null
					)
			: []
	};
};
