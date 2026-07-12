import type {
	EventTime,
	ParamConstraintPolicy,
	ParamEventBehaviour,
	ParamValue,
	UiAck,
	UiClient,
	UiCreatableUserItem,
	UiCreateUserItemInitialParam,
	UiDuplicateDependentUserItemInitialParam,
	UiEditIntent,
	UiEventBatch,
	UiEventDto,
	UiNodeDataDto,
	UiNodeDto,
	UiParamControlInfo,
	UiParameterControlMode,
	UiParameterControlSpec,
	UiParameterControlState,
	UiParamConstraints,
	UiParamDto,
	UiFileConstraints,
	UiProjectFileSpec,
	UiProjectFileLoadResult,
	UiProjectLoadOptions,
	UiProjectLoadRecovery,
	UiReferenceConstraints,
	UiReferenceTargets,
	UiReferenceRoot,
	UiScriptConfig,
	UiScriptManifest,
	UiScriptSource,
	UiScriptState,
	UiSnapshot,
	UiSubscriptionScope,
	UiTokenSuggestion,
	UiUserContextCandidate,
	UiUserContextValueType,
	UiParamControlCandidate,
	UiParamValueProjection
} from '../types';
import type { ParamValue as RustParamValue } from '../generated/rust_protocol/ParamValue';
import type { ScriptUiState as RustScriptState } from '../generated/rust_protocol/ScriptUiState';
import type { UiAck as RustUiAck } from '../generated/rust_protocol/UiAck';
import type { UiDeclaredDescriptionDescriptor as RustUiDeclaredDescriptionDescriptor } from '../generated/rust_protocol/UiDeclaredDescriptionDescriptor';
import type { UiEditIntent as RustUiEditIntent } from '../generated/rust_protocol/UiEditIntent';
import type { UiEventBatch as RustUiEventBatch } from '../generated/rust_protocol/UiEventBatch';
import type { UiEnumDefinition as RustUiSchemaEnumDefinition } from '../generated/rust_protocol/UiEnumDefinition';
import type { UiEnumVariantDefinition as RustUiSchemaEnumVariantDefinition } from '../generated/rust_protocol/UiEnumVariantDefinition';
import type { UiNodeDto as RustUiNodeDto } from '../generated/rust_protocol/UiNodeDto';
import type { UiNodeTypeDescriptor as RustUiNodeTypeDescriptor } from '../generated/rust_protocol/UiNodeTypeDescriptor';
import type { UiParamControlInfoDto as RustUiParamControlInfoResponse } from '../generated/rust_protocol/UiParamControlInfoDto';
import type { UiParamControlInfoRequest as RustParamControlInfoRequest } from '../generated/rust_protocol/UiParamControlInfoRequest';
import type { UiParamDto as RustUiParamDto } from '../generated/rust_protocol/UiParamDto';
import type { UiProjectPathDto as RustProjectPathResponse } from '../generated/rust_protocol/UiProjectPathDto';
import type { UiProjectPathRequest as RustProjectPathRequest } from '../generated/rust_protocol/UiProjectPathRequest';
import type { UiProjectUploadRequest as RustProjectUploadRequest } from '../generated/rust_protocol/UiProjectUploadRequest';
import type { UiReferenceTargetsDto as RustReferenceTargetsResponse } from '../generated/rust_protocol/UiReferenceTargetsDto';
import type { UiReferenceTargetsRequest as RustReferenceTargetsRequest } from '../generated/rust_protocol/UiReferenceTargetsRequest';
import type { UiReplayRequest as RustReplayRequest } from '../generated/rust_protocol/UiReplayRequest';
import type { UiScriptConfigRequest as RustScriptConfigRequest } from '../generated/rust_protocol/UiScriptConfigRequest';
import type { UiScriptReloadRequest as RustScriptReloadRequest } from '../generated/rust_protocol/UiScriptReloadRequest';
import type { UiScriptStateRequest as RustScriptStateRequest } from '../generated/rust_protocol/UiScriptStateRequest';
import type { UiSnapshot as RustUiSnapshot } from '../generated/rust_protocol/UiSnapshot';
import type { UiSnapshotRequest as RustSnapshotRequest } from '../generated/rust_protocol/UiSnapshotRequest';
import type { UiSubscriptionScope as RustUiSubscriptionScope } from '../generated/rust_protocol/UiSubscriptionScope';
import type { JsonValue as RustJsonValue } from '../generated/rust_protocol/serde_json/JsonValue';
import { isCssUnit } from '../css-value';
import { wholeGraphScope } from '../types';
import { getUiClientInstanceId } from './client-instance';

const DEFAULT_BASE_URL = 'http://localhost:7010/api/ui';
const DEFAULT_POLL_INTERVAL_MS = 150;
const DEFAULT_POST_TIMEOUT_MS = 15000;
const PROJECT_LOAD_TIMEOUT_MS = 60000;

interface HttpClientOptions {
	baseUrl?: string;
	pollIntervalMs?: number;
	fetchImpl?: typeof fetch;
}

interface PostJsonOptions {
	timeoutMs?: number;
}

export type RustScope = RustUiSubscriptionScope;
export type { RustUiEventBatch };

type RustUiEventDto = RustUiEventBatch['events'][number];

const fromRustProjectLoadRecovery = (recovery: unknown): UiProjectLoadRecovery | undefined => {
	if (!isRecord(recovery)) {
		return undefined;
	}

	const rawProblems = Array.isArray(recovery.problems) ? recovery.problems : [];
	const problems = rawProblems
		.map((problem): UiProjectLoadRecovery['problems'][number] | null => {
			if (!isRecord(problem)) {
				return null;
			}
			const message = typeof problem.message === 'string' ? problem.message.trim() : '';
			if (message.length === 0) {
				return null;
			}
			const stage =
				typeof problem.stage === 'string' && problem.stage.trim().length > 0
					? problem.stage.trim()
					: 'project_load';
			return { stage, message };
		})
		.filter((problem): problem is UiProjectLoadRecovery['problems'][number] => problem !== null);

	return problems.length > 0 ? { problems } : undefined;
};

const fromRustProjectPathResponse = (
	endpoint: string,
	response: RustProjectPathResponse
): UiProjectFileLoadResult => {
	if (typeof response.path !== 'string' || response.path.trim().length === 0) {
		throw new Error(`POST ${endpoint} returned an invalid project path`);
	}
	return {
		path: response.path,
		ui_state: response.ui_state,
		recovery: fromRustProjectLoadRecovery(response.recovery)
	};
};

const toRustJsonValue = (value: unknown): RustJsonValue | undefined => {
	if (value === undefined) {
		return undefined;
	}
	if (
		value === null ||
		typeof value === 'string' ||
		typeof value === 'boolean' ||
		(typeof value === 'number' && Number.isFinite(value))
	) {
		return value;
	}
	if (Array.isArray(value)) {
		return value.map((entry) => toRustJsonValue(entry) ?? null);
	}
	if (!isRecord(value)) {
		return undefined;
	}

	const output: { [key: string]: RustJsonValue } = {};
	for (const [key, entry] of Object.entries(value)) {
		const jsonEntry = toRustJsonValue(entry);
		if (jsonEntry !== undefined) {
			output[key] = jsonEntry;
		}
	}
	return output;
};
type RustScriptConfig = RustScriptConfigRequest['config'];
type RustScriptSource = RustScriptConfig['source'];
type RustCreatableUserItem = NonNullable<RustUiNodeDto['creatable_user_items']>[number];

const isRecord = (value: unknown): value is Record<string, unknown> =>
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

const fromRustParamValue = (value: unknown): ParamValue => {
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

const toRustParamValue = (value: ParamValue): RustParamValue => {
	switch (value.kind) {
		case 'trigger':
			return { Trigger: [] };
		case 'int':
			return { Int: value.value };
		case 'float':
			return { Float: value.value };
		case 'str':
			return { Str: value.value };
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

const fromRustProjectFileSpec = (projectFile: unknown): UiProjectFileSpec => {
	if (!isRecord(projectFile)) {
		return {
			display_name: 'Project',
			extension: 'json',
			current_path: null
		};
	}

	const displayName =
		typeof projectFile.display_name === 'string' && projectFile.display_name.trim().length > 0
			? projectFile.display_name.trim()
			: 'Project';
	const extension =
		typeof projectFile.extension === 'string' && projectFile.extension.trim().length > 0
			? projectFile.extension.trim().replace(/^\.+/, '').toLowerCase()
			: 'json';
	const currentPath =
		typeof projectFile.current_path === 'string' && projectFile.current_path.trim().length > 0
			? projectFile.current_path.trim()
			: null;

	return {
		display_name: displayName,
		extension: extension.length > 0 ? extension : 'json',
		current_path: currentPath
	};
};

const fromRustScriptSource = (source: unknown): UiScriptSource => {
	if (isRecord(source)) {
		if (source.kind === 'projectFile') {
			return { kind: 'projectFile', path: String(source.path ?? '') };
		}
		if (source.kind === 'inline') {
			return { kind: 'inline', text: String(source.text ?? '') };
		}
	}
	return { kind: 'inline', text: '' };
};

const toRustScriptSource = (source: UiScriptSource): RustScriptSource => {
	if (source.kind === 'projectFile') {
		return { kind: 'projectFile', path: source.path };
	}
	return { kind: 'inline', text: source.text };
};

const fromRustScriptConfig = (config: unknown): UiScriptConfig => {
	if (!isRecord(config)) {
		return {
			source: { kind: 'inline', text: '' }
		};
	}

	return {
		source: fromRustScriptSource(config.source)
	};
};

const toRustScriptConfig = (config: UiScriptConfig): RustScriptConfig => ({
	source: toRustScriptSource(config.source)
});

const fromRustScriptManifest = (
	manifest: RustScriptState['manifest']
): UiScriptManifest | undefined => {
	if (!isRecord(manifest)) {
		return undefined;
	}

	const apiVersion =
		typeof manifest.api_version === 'number' && Number.isFinite(manifest.api_version)
			? Math.max(1, Math.round(manifest.api_version))
			: undefined;
	if (apiVersion === undefined) {
		return undefined;
	}

	const exports = Array.isArray(manifest.exports)
		? manifest.exports
				.map((value) => {
					const signature = isRecord(value.signature) ? value.signature : {};
					const argsRaw = signature.args;
					const args = Array.isArray(argsRaw)
						? argsRaw.filter((entry): entry is string => typeof entry === 'string')
						: [];
					const returns =
						typeof signature.returns === 'string' && signature.returns.length > 0
							? signature.returns
							: undefined;
					return {
						name: typeof value.name === 'string' ? value.name : '',
						signature: { args, returns }
					};
				})
				.filter((value) => value.name.length > 0)
		: [];

	return {
		api_version: apiVersion,
		update_rate_hz:
			typeof manifest.update_rate_hz === 'number' && Number.isFinite(manifest.update_rate_hz)
				? Math.max(1, Math.round(manifest.update_rate_hz))
				: undefined,
		exports
	};
};

const fromRustScriptState = (state: RustScriptState): UiScriptState => ({
	config: fromRustScriptConfig(state.config),
	effective_update_rate_hz:
		typeof state.effective_update_rate_hz === 'number' &&
		Number.isFinite(state.effective_update_rate_hz)
			? Math.max(1, Math.round(state.effective_update_rate_hz))
			: undefined,
	export_names: Array.isArray(state.export_names)
		? state.export_names.filter((name): name is string => typeof name === 'string')
		: [],
	manifest: fromRustScriptManifest(state.manifest)
});

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

const fromRustConstraints = (
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

const toRustConstraints = (
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
): { target_id: number; direct: boolean; projections: UiParamValueProjection[] } | null => {
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

const fromRustControlState = (control: unknown): UiParameterControlState => {
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
): { enums: UiSnapshot['schema']['enums']; enumOptionsById: EnumOptionsById } => {
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
		return { kind: 'parameter', param: fromRustParam(data.param, enumOptionsById) };
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
				return { ...op, nodes: rawNodes.map((n) => fromRustEventNodeSnapshot(n)) };
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

const fromRustAck = (ack: RustUiAck): UiAck => ({
	success: ack.success,
	status: ack.status,
	error_code: ack.error_code ?? undefined,
	error_message: ack.error_message ?? undefined,
	earliest_event_time: ack.earliest_event_time ?? undefined,
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

const formatError = (context: string, response: Response, body: unknown): Error => {
	const suffix = isRecord(body) && typeof body.error === 'string' ? `: ${body.error}` : '';
	return Object.assign(
		new Error(`${context} failed with ${response.status} ${response.statusText}${suffix}`),
		{
			status: response.status,
			statusText: response.statusText,
			body
		}
	);
};

export const createHttpUiClient = (options: HttpClientOptions = {}): UiClient => {
	const fetchImpl = options.fetchImpl ?? fetch;
	const baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, '');
	const pollIntervalMs = Math.max(50, options.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS);
	const clientInstanceId = getUiClientInstanceId();

	const postJson = async <TResponse>(
		path: string,
		payload: unknown,
		options: PostJsonOptions = {}
	): Promise<TResponse> => {
		const timeoutMs = Math.max(1000, options.timeoutMs ?? DEFAULT_POST_TIMEOUT_MS);
		const controller = typeof AbortController !== 'undefined' ? new AbortController() : null;
		const timeoutId =
			controller !== null
				? setTimeout(() => {
						controller.abort();
					}, timeoutMs)
				: null;

		try {
			const response = await fetchImpl(`${baseUrl}${path}`, {
				method: 'POST',
				headers: {
					'content-type': 'application/json',
					'x-gc-ui-client-instance': clientInstanceId
				},
				body: JSON.stringify(payload),
				signal: controller?.signal
			});

			let body: unknown = undefined;
			const text = await response.text();
			if (text.length > 0) {
				try {
					body = JSON.parse(text) as unknown;
				} catch {
					body = text;
				}
			}

			if (!response.ok) {
				throw formatError(`POST ${path}`, response, body);
			}

			return body as TResponse;
		} catch (error) {
			if (controller?.signal.aborted) {
				throw new Error(`POST ${path} timed out after ${Math.round(timeoutMs / 1000)}s`);
			}
			throw error;
		} finally {
			if (timeoutId !== null) {
				clearTimeout(timeoutId);
			}
		}
	};

	const client: UiClient = {
		async snapshot(scope: UiSubscriptionScope = wholeGraphScope): Promise<UiSnapshot> {
			const totalStart = typeof performance !== 'undefined' ? performance.now() : Date.now();
			const request: RustSnapshotRequest = {
				scope: toRustScope(scope),
				cancel_active_edit_session: true
			};
			const fetchStart = typeof performance !== 'undefined' ? performance.now() : Date.now();
			const response = await fetchImpl(`${baseUrl}/snapshot`, {
				method: 'POST',
				headers: {
					'content-type': 'application/json',
					'x-gc-ui-client-instance': clientInstanceId
				},
				body: JSON.stringify(request)
			});
			const fetchMs =
				(typeof performance !== 'undefined' ? performance.now() : Date.now()) - fetchStart;

			const readStart = typeof performance !== 'undefined' ? performance.now() : Date.now();
			const text = await response.text();
			const readMs =
				(typeof performance !== 'undefined' ? performance.now() : Date.now()) - readStart;

			const parseStart = typeof performance !== 'undefined' ? performance.now() : Date.now();
			let body: unknown = undefined;
			if (text.length > 0) {
				try {
					body = JSON.parse(text) as unknown;
				} catch {
					body = text;
				}
			}

			if (!response.ok) {
				throw formatError(`POST /snapshot`, response, body);
			}

			const snapshot = body as RustUiSnapshot;
			const parsed = fromRustSnapshot(snapshot);
			const parseMs =
				(typeof performance !== 'undefined' ? performance.now() : Date.now()) - parseStart;
			const totalMs =
				(typeof performance !== 'undefined' ? performance.now() : Date.now()) - totalStart;

			(parsed as any).__timings = { fetchMs, readMs, parseMs, totalMs, bytes: text.length };
			return parsed;
		},

		subscribe(
			scope: UiSubscriptionScope,
			from: EventTime | undefined,
			onBatch: (batch: UiEventBatch) => void
		): () => void {
			let cursor = from;
			let active = true;
			let inFlight = false;

			const poll = async (): Promise<void> => {
				if (!active || inFlight) {
					return;
				}
				inFlight = true;
				try {
					const batch = await client.replay(scope, cursor);
					if (!active) {
						return;
					}
					if (batch.events.length > 0 || batch.runtime) {
						onBatch(batch);
					}
					if (batch.to) {
						cursor = batch.to;
					}
				} catch (error) {
					console.error('ui subscribe replay polling failed', error);
				} finally {
					inFlight = false;
				}
			};

			const timer = setInterval(() => {
				void poll();
			}, pollIntervalMs);
			void poll();

			return () => {
				active = false;
				clearInterval(timer);
			};
		},

		async sendIntent(intent: UiEditIntent): Promise<UiAck> {
			const ack = await postJson<RustUiAck>('/intent', toRustIntent(intent));
			return fromRustAck(ack);
		},

		async sendIntents(intents: UiEditIntent[]): Promise<UiAck[]> {
			if (intents.length === 0) {
				return [];
			}
			if (intents.length === 1) {
				return [await client.sendIntent(intents[0])];
			}
			const payload = intents.map((intent) => toRustIntent(intent));
			const acks = await postJson<RustUiAck[]>('/intent/batch', payload);
			if (!Array.isArray(acks)) {
				throw new Error('POST /intent/batch returned invalid payload');
			}
			return acks.map(fromRustAck);
		},

		async replay(scope: UiSubscriptionScope, from?: EventTime): Promise<UiEventBatch> {
			const request: RustReplayRequest = { scope: toRustScope(scope), from };
			const batch = await postJson<RustUiEventBatch>('/replay', request);
			return fromRustEventBatch(batch);
		},

		async referenceTargets(paramNodeId: number): Promise<UiReferenceTargets> {
			const request: RustReferenceTargetsRequest = { param: paramNodeId };
			const response = await postJson<RustReferenceTargetsResponse>('/reference-targets', request);
			return fromRustReferenceTargets(response);
		},

		async paramControlInfo(paramNodeId: number): Promise<UiParamControlInfo> {
			const request: RustParamControlInfoRequest = { param: paramNodeId };
			const response = await postJson<RustUiParamControlInfoResponse>(
				'/param-control-info',
				request
			);
			return fromRustParamControlInfo(response);
		},

		async scriptState(nodeId: number): Promise<UiScriptState> {
			const request: RustScriptStateRequest = { node: nodeId };
			const response = await postJson<RustScriptState>('/script-state', request);
			return fromRustScriptState(response);
		},

		async setScriptConfig(
			nodeId: number,
			config: UiScriptConfig,
			forceReload = false
		): Promise<void> {
			const request: RustScriptConfigRequest = {
				node: nodeId,
				config: toRustScriptConfig(config),
				force_reload: forceReload
			};
			await postJson<{ ok?: boolean }>('/script-config', request);
		},

		async reloadScript(nodeId: number): Promise<void> {
			const request: RustScriptReloadRequest = { node: nodeId };
			await postJson<{ ok?: boolean }>('/script-reload', request);
		},

		async projectNew(): Promise<void> {
			await postJson<{ ok?: boolean }>('/project-new', {});
		},

		async projectSave(path: string, uiState?: unknown): Promise<void> {
			const request: RustProjectPathRequest = { path };
			const jsonUiState = toRustJsonValue(uiState);
			if (jsonUiState !== undefined) {
				request.ui_state = jsonUiState;
			}
			await postJson<{ ok?: boolean }>('/project-save', request);
		},

		async projectLoad(
			path: string,
			options: UiProjectLoadOptions = {}
		): Promise<UiProjectFileLoadResult> {
			const request: RustProjectPathRequest = { path };
			if (options.recover === true) {
				request.recover = true;
			}
			const response = await postJson<RustProjectPathResponse>('/project-load', request, {
				timeoutMs: PROJECT_LOAD_TIMEOUT_MS
			});
			return fromRustProjectPathResponse('/project-load', response);
		},

		async projectUploadLoad(
			fileName: string,
			contents: string,
			options: UiProjectLoadOptions = {}
		): Promise<UiProjectFileLoadResult> {
			const request: RustProjectUploadRequest = {
				file_name: fileName,
				contents
			};
			if (options.recover === true) {
				request.recover = true;
			}
			const response = await postJson<RustProjectPathResponse>('/project-upload-load', request, {
				timeoutMs: PROJECT_LOAD_TIMEOUT_MS
			});
			return fromRustProjectPathResponse('/project-upload-load', response);
		}
	};

	return client;
};
