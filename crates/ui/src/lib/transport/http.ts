import type {
	EventTime,
	ParamConstraintPolicy,
	ParamEventBehaviour,
	ParamValue,
	UiAck,
	UiClient,
	UiEditIntent,
	UiEventBatch,
	UiEventDto,
	UiNodeDataDto,
	UiNodeDto,
	UiHistoryState,
	UiNodeMetaDto,
	UiParamConstraints,
	UiParamDto,
	UiSnapshot,
	UiSubscriptionScope
} from '../types';
import { wholeGraphScope } from '../types';

const DEFAULT_BASE_URL = 'http://localhost:7010/api/ui';
const DEFAULT_POLL_INTERVAL_MS = 150;

interface HttpClientOptions {
	baseUrl?: string;
	pollIntervalMs?: number;
	fetchImpl?: typeof fetch;
}

export type RustScope = string | { subtree: { root: number; max_depth: number } };

export interface RustSnapshotRequest {
	scope: RustScope;
}

export interface RustReplayRequest {
	scope: RustScope;
	from?: EventTime;
}

export type RustParamValue =
	| string
	| {
			[key: string]: unknown;
	  };

interface RustUiParamDto {
	value: RustParamValue;
	event_behaviour: ParamEventBehaviour;
	read_only: boolean;
	constraints: {
		min?: number;
		max?: number;
		step?: number;
		step_base?: number;
		enum_options?: Array<{
			variant_id: string;
			value: RustParamValue;
			label: string;
			tags?: string[];
			ordering?: number;
		}>;
		policy: ParamConstraintPolicy;
	};
	ui_hints?: {
		widget?: string;
		unit?: string;
	};
}

interface RustUiNodeDto {
	node_id: number;
	uuid: string;
	decl_id: string;
	node_type: string;
	meta: Omit<UiNodeMetaDto, 'tags'> & { tags?: string[] };
	data: { kind: 'parameter'; param: RustUiParamDto } | { kind: 'node'; node_type: string };
	user_role?: 'regular' | 'itemRoot';
	user_item_kind?: string;
	accepted_user_item_kinds?: string[];
	creatable_user_items?: Array<{ node_type: string; item_kind: string; label: string }>;
	children: number[];
}

export interface RustUiSnapshot {
	protocol_version: string;
	scope: RustScope;
	at: EventTime;
	nodes: RustUiNodeDto[];
	schema: Partial<UiSnapshot['schema']>;
	history?: UiHistoryState;
}

type RustUiEventDto =
	| (Omit<UiEventDto, 'kind'> & {
			kind:
				| {
						kind: 'paramChanged';
						param: number;
						old_value: RustParamValue;
						new_value: RustParamValue;
				  }
				| Exclude<UiEventDto['kind'], { kind: 'paramChanged' }>;
	  })
	| (Omit<UiEventDto, 'kind'> & {
			kind: UiEventDto['kind']['kind'];
			param?: number;
			old_value?: RustParamValue;
			new_value?: RustParamValue;
			parent?: number;
			child?: number;
			decl_id?: string;
			old?: number;
			new?: number;
			old_parent?: number;
			new_parent?: number;
			node?: number;
			patch?: unknown;
			topic?: string;
			origin?: number;
			payload?: unknown;
	  });

export interface RustUiEventBatch {
	from?: EventTime;
	to?: EventTime;
	events: RustUiEventDto[];
}

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

const fromRustReferencePayload = (payload: unknown): { uuid: string; cached_id?: number } => {
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
		return { uuid, cached_id };
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
	if ('Enum' in value) {
		return { kind: 'enum', value: String(value.Enum ?? '') };
	}
	if ('Bool' in value) {
		return { kind: 'bool', value: Boolean(value.Bool) };
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
			value: [
				Number(vec[0] ?? 0),
				Number(vec[1] ?? 0),
				Number(vec[2] ?? 0),
				Number(vec[3] ?? 1)
			]
		};
	}
	if ('Reference' in value) {
		const reference = fromRustReferencePayload(value.Reference);
		return {
			kind: 'reference',
			uuid: reference.uuid,
			cached_id: reference.cached_id
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
		case 'enum':
			return { Enum: value.value };
		case 'bool':
			return { Bool: value.value };
		case 'vec2':
			return { Vec2: value.value };
		case 'vec3':
			return { Vec3: value.value };
		case 'color':
			return { Color: value.value };
		case 'reference':
			return { Reference: { uuid: value.uuid } };
	}
};

const fromRustConstraints = (constraints: RustUiParamDto['constraints']): UiParamConstraints => ({
	min: constraints.min,
	max: constraints.max,
	step: constraints.step,
	step_base: constraints.step_base,
	enum_options: (constraints.enum_options ?? []).map((option) => ({
		variant_id: option.variant_id,
		value: fromRustParamValue(option.value),
		label: option.label,
		tags: [...(option.tags ?? [])],
		ordering: option.ordering
	})),
	policy: constraints.policy
});

const fromRustParam = (param: RustUiParamDto): UiParamDto => ({
	value: fromRustParamValue(param.value),
	event_behaviour: param.event_behaviour,
	read_only: param.read_only,
	constraints: fromRustConstraints(param.constraints),
	ui_hints: { ...(param.ui_hints ?? {}) }
});

const fromRustNodeData = (data: RustUiNodeDto['data']): UiNodeDataDto => {
	if (data.kind === 'parameter') {
		return { kind: 'parameter', param: fromRustParam(data.param) };
	}
	return { kind: 'node', node_type: data.node_type };
};

const fromRustNode = (node: RustUiNodeDto): UiNodeDto => ({
	node_id: node.node_id,
	uuid: node.uuid,
	decl_id: node.decl_id,
	node_type: node.node_type,
	meta: { ...node.meta, tags: [...(node.meta.tags ?? [])] },
	data: fromRustNodeData(node.data),
	user_role: node.user_role ?? 'regular',
	user_item_kind: node.user_item_kind ?? node.node_type,
	accepted_user_item_kinds: [...(node.accepted_user_item_kinds ?? [])],
	creatable_user_items: [...(node.creatable_user_items ?? [])],
	children: [...node.children]
});

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
		return event as UiEventDto;
	}

	if (typeof nestedKind !== 'string') {
		return event as UiEventDto;
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

export const fromRustSnapshot = (snapshot: RustUiSnapshot): UiSnapshot => ({
	protocol_version: snapshot.protocol_version,
	scope: fromRustScope(snapshot.scope),
	at: snapshot.at,
	nodes: snapshot.nodes.map(fromRustNode),
	schema: {
		node_types: snapshot.schema.node_types ?? [],
		enums: snapshot.schema.enums ?? []
	},
	history: snapshot.history ?? {
		can_undo: false,
		can_redo: false,
		undo_len: 0,
		redo_len: 0,
		active_edit_session: false
	}
});

export const fromRustEventBatch = (batch: RustUiEventBatch): UiEventBatch => ({
	from: batch.from,
	to: batch.to,
	events: batch.events.map(fromRustEvent)
});

export const toRustIntent = (intent: UiEditIntent): unknown => {
	if (intent.kind === 'setParam') {
		return {
			...intent,
			value: toRustParamValue(intent.value)
		};
	}
	return intent;
};

const formatError = (context: string, response: Response, body: unknown): Error => {
	const suffix = isRecord(body) && typeof body.error === 'string' ? `: ${body.error}` : '';
	return new Error(`${context} failed with ${response.status} ${response.statusText}${suffix}`);
};

export const createHttpUiClient = (options: HttpClientOptions = {}): UiClient => {
	const fetchImpl = options.fetchImpl ?? fetch;
	const baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, '');
	const pollIntervalMs = Math.max(50, options.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS);

	const postJson = async <TResponse>(path: string, payload: unknown): Promise<TResponse> => {
		const response = await fetchImpl(`${baseUrl}${path}`, {
			method: 'POST',
			headers: {
				'content-type': 'application/json'
			},
			body: JSON.stringify(payload)
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
	};

	const client: UiClient = {
		async snapshot(scope: UiSubscriptionScope = wholeGraphScope): Promise<UiSnapshot> {
			const request: RustSnapshotRequest = { scope: toRustScope(scope) };
			const snapshot = await postJson<RustUiSnapshot>('/snapshot', request);
			return fromRustSnapshot(snapshot);
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
					if (batch.events.length > 0) {
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
			const ack = await postJson<UiAck>('/intent', toRustIntent(intent));
			return ack;
		},

		async replay(scope: UiSubscriptionScope, from?: EventTime): Promise<UiEventBatch> {
			const request: RustReplayRequest = { scope: toRustScope(scope), from };
			const batch = await postJson<RustUiEventBatch>('/replay', request);
			return fromRustEventBatch(batch);
		}
	};

	return client;
};
