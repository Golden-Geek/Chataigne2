import type { EventTime, UiNodeDto } from 'golden_ui';
import { compareEventTime } from 'golden_ui/store/session/event-time';
import { createSpatializerAuthoring, type SpatializerSession } from './authoring';
import type { SpatializerSpace } from './generated/SpatializerSpace';
import type { SpatializerEdit } from './generated/SpatializerEdit';
import type { SpatialEndpoint, SpatializerCanvas, Point2, RadiusDragKind } from './types';
import { clampPositive } from './projection.svelte';
import { graphPositionToSpatialPosition } from './geometry.svelte';

type Kind = 'position' | RadiusDragKind;
type Ticket = ReturnType<ReturnType<typeof createSpatializerAuthoring>['gesture']>;
interface Source {
	session: SpatializerSession | null;
	module: UiNodeDto | null;
	space: SpatializerSpace;
	rawEndpoints: SpatialEndpoint[];
	graphCanvas: SpatializerCanvas | null;
	worldSvg: SVGSVGElement | null;
	selectEndpoint(endpoint: SpatialEndpoint): void;
}
interface Drag {
	token: number;
	pointerId: number | null;
	endpoint: SpatialEndpoint;
	kind: Kind;
	ticket: Ticket;
	frame: number | null;
	dirty: boolean;
	startPointer: Point2;
	startPosition: Point2;
	startRadius: number;
	startDistance: number;
	latestPosition: Point2;
	latestRadius: number;
}
type Preview = {
	token: number;
	endpoint: SpatialEndpoint;
	current(): boolean;
	finished: boolean;
	boundary?: EventTime;
} & ({ kind: 'position'; value: Point2 } | { kind: RadiusDragKind; value: number });
const arrow = (key: string, step: number): Point2 | null =>
	key === 'ArrowRight'
		? { x: step, y: 0 }
		: key === 'ArrowLeft'
			? { x: -step, y: 0 }
			: key === 'ArrowDown'
				? { x: 0, y: step }
				: key === 'ArrowUp'
					? { x: 0, y: -step }
					: null;
const identity = (endpoint: UiNodeDto) => ({
	node: endpoint.node_id,
	uuid: endpoint.uuid
});
const writable = (endpoint: SpatialEndpoint, kind: Kind) =>
	kind === 'position'
		? endpoint.positionWritable
		: kind === 'radius'
			? endpoint.radiusWritable
			: endpoint.freezeRadiusWritable;

export function createSpatializerInteraction(getSource: () => Source) {
	const authoring = createSpatializerAuthoring(getSource);
	let activeDrag = $state.raw<Drag | null>(null),
		previews = $state.raw(new Map<string, Preview>());
	let message = $state(''),
		serial = 0,
		feedbackToken = 0,
		disposed = false;
	let feedbackOwner = $state.raw<(() => boolean) | null>(null);
	const startFeedback = () => {
		const source = getSource(),
			session = source.session,
			generation = session?.snapshotGeneration,
			module = source.module,
			space = source.space;
		const node = module?.node_id,
			uuid = module?.uuid;
		const current = () =>
			!disposed &&
			getSource().session === session &&
			session?.snapshotGeneration === generation &&
			getSource().module?.node_id === node &&
			getSource().module?.uuid === uuid &&
			getSource().space === space;
		const token = ++serial;
		feedbackToken = token;
		feedbackOwner = current;
		message = '';
		return { token, current };
	};
	const previewKey = (endpoint: SpatialEndpoint, kind: Kind) => kind + ':' + endpoint.key;
	const remove = (token: number) => {
		const next = new Map(previews);
		for (const [key, value] of next) if (value.token === token) next.delete(key);
		previews = next;
	};
	const updatePreview = (drag: Drag) => {
		const shared = {
			token: drag.token,
			endpoint: drag.endpoint,
			current: () => drag.ticket.ownerCurrent,
			finished: false
		};
		const value: Preview =
			drag.kind === 'position'
				? { ...shared, kind: 'position', value: { ...drag.latestPosition } }
				: { ...shared, kind: drag.kind, value: drag.latestRadius };
		previews = new Map(previews).set(previewKey(drag.endpoint, drag.kind), value);
	};
	const positions = $derived.by(() => {
		const result: Record<string, Point2> = {};
		for (const value of previews.values())
			if (value.kind === 'position') result[value.endpoint.key] = value.value;
		return result;
	});
	const radii = $derived.by(() => {
		const result: Record<string, number> = {};
		for (const value of previews.values())
			if (value.kind === 'radius') result[value.endpoint.key] = value.value;
		return result;
	});
	const freezes = $derived.by(() => {
		const result: Record<string, number> = {};
		for (const value of previews.values())
			if (value.kind === 'freezeRadius') result[value.endpoint.key] = value.value;
		return result;
	});
	const release = (drag: Drag) => {
		if (drag.frame !== null) {
			cancelAnimationFrame(drag.frame);
			drag.frame = null;
		}
		const svg = getSource().worldSvg;
		if (drag.pointerId !== null && svg?.hasPointerCapture(drag.pointerId))
			svg.releasePointerCapture(drag.pointerId);
	};
	const cancel = (drag: Drag) => {
		if (activeDrag === drag) activeDrag = null;
		release(drag);
		remove(drag.token);
		return drag.ticket.cancel();
	};
	$effect(() => {
		if (feedbackOwner && !feedbackOwner()) {
			feedbackOwner = null;
			feedbackToken = ++serial;
			message = '';
		}
		const at = getSource().session?.graph.state.lastEventTime;
		if (activeDrag && !activeDrag.ticket.current) void cancel(activeDrag).catch(() => undefined);
		let next: Map<string, Preview> | null = null;
		for (const [key, value] of previews) {
			if (
				!value.current() ||
				(value.finished && (!value.boundary || (at && compareEventTime(at, value.boundary) >= 0)))
			) {
				next ??= new Map(previews);
				next.delete(key);
			}
		}
		if (next) previews = next;
	});
	const flush = (drag: Drag) => {
		if (!drag.dirty) return;
		drag.dirty = false;
		const edit: SpatializerEdit =
			drag.kind === 'position'
				? {
						kind: 'move',
						moves: [
							{
								endpoint: identity(drag.endpoint.node),
								position: [drag.latestPosition.x, drag.latestPosition.y]
							}
						]
					}
				: {
						kind: 'setRadius',
						endpoint: identity(drag.endpoint.node),
						field: drag.kind,
						value: drag.latestRadius
					};
		drag.ticket.offer(edit);
	};
	const schedule = (drag: Drag) => {
		drag.dirty = true;
		if (drag.frame !== null) return;
		drag.frame = requestAnimationFrame(() => {
			drag.frame = null;
			try {
				flush(drag);
			} catch {
				void cancel(drag).catch(() => undefined);
			}
		});
	};
	const begin = (
		endpoint: SpatialEndpoint,
		kind: Kind,
		pointer: Point2,
		pointerId: number | null = null
	): boolean => {
		if (disposed || activeDrag || !writable(endpoint, kind)) return false;
		const field =
			kind === 'position'
				? endpoint.positionParam
				: kind === 'radius'
					? endpoint.radiusParam
					: endpoint.freezeRadiusParam;
		if (!field) return false;
		const { token, current: feedbackCurrent } = startFeedback();
		try {
			const ticket = authoring.gesture(endpoint.node, field, () => {
				const current = previews.get(previewKey(endpoint, kind));
				if (current?.token === token && current.current() && feedbackToken === token)
					message = 'Endpoint edit failed.';
				remove(token);
				if (activeDrag?.token === token) {
					release(activeDrag);
					activeDrag = null;
				}
			});
			const radius =
				kind === 'radius'
					? (endpoint.radius ?? 0)
					: kind === 'freezeRadius'
						? (endpoint.freezeRadius ?? 0)
						: 0;
			activeDrag = {
				token,
				pointerId,
				endpoint,
				kind,
				ticket,
				frame: null,
				dirty: false,
				startPointer: { ...pointer },
				startPosition: { x: endpoint.x, y: endpoint.y },
				startRadius: radius,
				startDistance: Math.hypot(pointer.x - endpoint.x, pointer.y - endpoint.y),
				latestPosition: { x: endpoint.x, y: endpoint.y },
				latestRadius: radius
			};
			getSource().selectEndpoint(endpoint);
			return true;
		} catch {
			if (feedbackCurrent()) message = 'Endpoint editing is unavailable.';
			return false;
		}
	};
	const update = (point: Point2): void => {
		const drag = activeDrag;
		if (!drag || !drag.ticket.current) return;
		if (drag.kind === 'position')
			drag.latestPosition = {
				x: drag.startPosition.x + point.x - drag.startPointer.x,
				y: drag.startPosition.y + point.y - drag.startPointer.y
			};
		else
			drag.latestRadius = clampPositive(
				drag.startRadius +
					Math.hypot(point.x - drag.startPosition.x, point.y - drag.startPosition.y) -
					drag.startDistance
			);
		updatePreview(drag);
		schedule(drag);
	};
	const finish = async (cancelled = false): Promise<void> => {
		const drag = activeDrag;
		if (!drag) return;
		activeDrag = null;
		release(drag);
		if (cancelled) {
			await cancel(drag);
			return;
		}
		try {
			flush(drag);
			const boundary = await drag.ticket.finish();
			const key = previewKey(drag.endpoint, drag.kind),
				previous = previews.get(key);
			if (previous?.token === drag.token)
				previews = new Map(previews).set(key, { ...previous, finished: true, boundary });
		} catch (error) {
			remove(drag.token);
			if (drag.ticket.ownerCurrent && feedbackToken === drag.token)
				message = 'Endpoint edit failed.';
			throw error;
		}
	};
	const keyboard = async (
		endpoint: SpatialEndpoint,
		key: string,
		shift = false,
		kind: Kind = 'position'
	): Promise<boolean> => {
		if (key === 'Enter' || key === ' ') {
			getSource().selectEndpoint(endpoint);
			return true;
		}
		const delta = arrow(key, shift ? 2.5 : 0.25);
		if (!delta) return false;
		if (!writable(endpoint, kind)) {
			startFeedback();
			message = 'Endpoint is read-only.';
			return false;
		}
		getSource().selectEndpoint(endpoint);
		const { token, current: feedbackCurrent } = startFeedback();
		try {
			await authoring.run(
				kind === 'position'
					? { kind: 'nudge', endpoint: identity(endpoint.node), delta: [delta.x, delta.y] }
					: {
							kind: 'adjustRadius',
							endpoint: identity(endpoint.node),
							field: kind,
							delta: delta.x - delta.y
						}
			);
			return true;
		} catch (error) {
			if (feedbackToken === token && feedbackCurrent()) message = 'Endpoint edit failed.';
			throw error;
		}
	};
	const pointer = (event: PointerEvent): Point2 =>
		graphPositionToSpatialPosition(
			getSource().graphCanvas?.clientToWorld(event.clientX, event.clientY) ?? { x: 0, y: 0 }
		);
	return {
		get activeEndpointKey() {
			return activeDrag?.endpoint.key ?? null;
		},
		get positionPreviews() {
			return positions;
		},
		get radiusPreviews() {
			return radii;
		},
		get freezeRadiusPreviews() {
			return freezes;
		},
		get message() {
			return message;
		},
		begin,
		update,
		finish,
		keyboard,
		async move(moves: { endpoint: UiNodeDto; position: Point2 }[]) {
			const { token, current: feedbackCurrent } = startFeedback();
			try {
				return await authoring.run({
					kind: 'move',
					moves: moves.map(({ endpoint, position }) => ({
						endpoint: identity(endpoint),
						position: [position.x, position.y]
					}))
				});
			} catch (error) {
				if (feedbackToken === token && feedbackCurrent()) message = 'Endpoint edit failed.';
				throw error;
			}
		},
		async createTarget(position: Point2) {
			const { token, current: feedbackCurrent } = startFeedback();
			try {
				return await authoring.run({ kind: 'createTarget', position: [position.x, position.y] });
			} catch (error) {
				if (feedbackToken === token && feedbackCurrent()) message = 'Target creation failed.';
				throw error;
			}
		},
		startEndpointDrag(event: PointerEvent, endpoint: SpatialEndpoint, requested: Kind) {
			if (event.button !== 0) return;
			event.preventDefault();
			event.stopPropagation();
			getSource().graphCanvas?.focus();
			const kind =
				requested === 'position' && event.altKey
					? endpoint.radiusWritable
						? 'radius'
						: endpoint.freezeRadiusWritable
							? 'freezeRadius'
							: requested
					: requested;
			if (begin(endpoint, kind, pointer(event), event.pointerId))
				getSource().worldSvg?.setPointerCapture(event.pointerId);
		},
		selectEndpointWithKeyboard(
			event: KeyboardEvent,
			endpoint: SpatialEndpoint,
			kind: Kind = 'position'
		) {
			if (event.key !== 'Enter' && event.key !== ' ' && !arrow(event.key, 1)) return;
			event.preventDefault();
			event.stopPropagation();
			void keyboard(endpoint, event.key, event.shiftKey, kind).catch(() => undefined);
		},
		handleWorldPointerMove(event: PointerEvent) {
			if (activeDrag?.pointerId !== event.pointerId) return;
			event.preventDefault();
			update(pointer(event));
		},
		finishWorldPointer(event: PointerEvent) {
			if (activeDrag?.pointerId !== event.pointerId) return;
			event.preventDefault();
			void finish().catch(() => undefined);
		},
		cancelWorldPointer(event: PointerEvent) {
			if (activeDrag?.pointerId === event.pointerId) void finish(true).catch(() => undefined);
		},
		dispose() {
			disposed = true;
			if (activeDrag) void cancel(activeDrag).catch(() => undefined);
			authoring.dispose();
			previews = new Map();
		}
	};
}
export type SpatializerInteraction = ReturnType<typeof createSpatializerInteraction>;
