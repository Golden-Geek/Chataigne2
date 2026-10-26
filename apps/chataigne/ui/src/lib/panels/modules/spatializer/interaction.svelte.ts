import type { ParamValue } from 'golden_ui';
import { createUiEditSession, sendSetParamIntent } from 'golden_ui/store/ui-intents';
import type { SpatialEndpoint, SpatializerCanvas, Point2, RadiusDragKind } from './types';
import { clampPositive } from './projection.svelte';
import { graphPositionToSpatialPosition } from './geometry.svelte';

type UiEditSession = ReturnType<typeof createUiEditSession>;
type DragGesture =
	| {
			kind: 'position';
			pointerId: number;
			endpoint: SpatialEndpoint;
			editSession: UiEditSession;
			updateTail: Promise<void>;
			sendFrame: number | null;
			sendInFlight: boolean;
			sendDirty: boolean;
			latestPosition: Point2;
			startPointer: Point2;
			startPosition: Point2;
	  }
	| {
			kind: RadiusDragKind;
			pointerId: number;
			endpoint: SpatialEndpoint;
			editSession: UiEditSession;
			updateTail: Promise<void>;
			sendFrame: number | null;
			sendInFlight: boolean;
			sendDirty: boolean;
			latestRadius: number;
			startRadius: number;
			startDistance: number;
			center: Point2;
	  };

interface Source {
	rawEndpoints: SpatialEndpoint[];
	graphCanvas: SpatializerCanvas | null;
	worldSvg: SVGSVGElement | null;
	selectEndpoint(endpoint: SpatialEndpoint): void;
}
export function createSpatializerInteraction(getSource: () => Source) {
	let activeDrag = $state<DragGesture | null>(null);

	let positionPreviews = $state<Record<string, Point2>>({});

	let radiusPreviews = $state<Record<string, number>>({});

	let freezeRadiusPreviews = $state<Record<string, number>>({});
	let rawEndpoints = $derived(getSource().rawEndpoints);
	let graphCanvas = $derived(getSource().graphCanvas);
	let worldSvg = $derived(getSource().worldSvg);
	const selectEndpoint = (endpoint: SpatialEndpoint) => getSource().selectEndpoint(endpoint);
	$effect(() => {
		const rawByKey = new Map(rawEndpoints.map((endpoint) => [endpoint.key, endpoint]));
		let nextPositions = positionPreviews;
		let positionChanged = false;
		for (const [key, preview] of Object.entries(positionPreviews)) {
			const raw = rawByKey.get(key);
			if (raw && Math.abs(raw.x - preview.x) < 0.0001 && Math.abs(raw.y - preview.y) < 0.0001) {
				if (!positionChanged) {
					nextPositions = { ...nextPositions };
					positionChanged = true;
				}
				delete nextPositions[key];
			}
		}
		if (positionChanged) {
			positionPreviews = nextPositions;
		}

		let nextRadii = radiusPreviews;
		let radiusChanged = false;
		for (const [key, preview] of Object.entries(radiusPreviews)) {
			const raw = rawByKey.get(key);
			if (raw && raw.radius !== null && Math.abs(raw.radius - preview) < 0.0001) {
				if (!radiusChanged) {
					nextRadii = { ...nextRadii };
					radiusChanged = true;
				}
				delete nextRadii[key];
			}
		}
		if (radiusChanged) {
			radiusPreviews = nextRadii;
		}

		let nextFreezeRadii = freezeRadiusPreviews;
		let freezeRadiusChanged = false;
		for (const [key, preview] of Object.entries(freezeRadiusPreviews)) {
			const raw = rawByKey.get(key);
			if (raw && raw.freezeRadius !== null && Math.abs(raw.freezeRadius - preview) < 0.0001) {
				if (!freezeRadiusChanged) {
					nextFreezeRadii = { ...nextFreezeRadii };
					freezeRadiusChanged = true;
				}
				delete nextFreezeRadii[key];
			}
		}
		if (freezeRadiusChanged) {
			freezeRadiusPreviews = nextFreezeRadii;
		}
	});

	const clearPositionPreview = (key: string): void => {
		const next = { ...positionPreviews };
		delete next[key];
		positionPreviews = next;
	};

	const clearRadiusPreview = (key: string): void => {
		const next = { ...radiusPreviews };
		delete next[key];
		radiusPreviews = next;
	};

	const clearFreezeRadiusPreview = (key: string): void => {
		const next = { ...freezeRadiusPreviews };
		delete next[key];
		freezeRadiusPreviews = next;
	};

	const setEndpointPosition = (endpoint: SpatialEndpoint, position: Point2): Promise<boolean> => {
		if (
			endpoint.positionParam?.data.kind !== 'parameter' ||
			endpoint.positionParam.data.param.read_only
		) {
			return Promise.resolve(false);
		}
		const value: ParamValue =
			endpoint.positionKind === 'vec3'
				? { kind: 'vec3', value: [position.x, endpoint.middle, position.y] }
				: { kind: 'vec2', value: [position.x, position.y] };
		positionPreviews = { ...positionPreviews, [endpoint.key]: position };
		return sendSetParamIntent(
			endpoint.positionParam.node_id,
			value,
			endpoint.positionParam.data.param.event_behaviour
		)
			.then((success) => {
				if (!success) {
					clearPositionPreview(endpoint.key);
				}
				return success;
			})
			.catch(() => {
				clearPositionPreview(endpoint.key);
				return false;
			});
	};

	const setEndpointRadius = (endpoint: SpatialEndpoint, radius: number): Promise<boolean> => {
		if (
			endpoint.radiusParam?.data.kind !== 'parameter' ||
			endpoint.radiusParam.data.param.read_only
		) {
			return Promise.resolve(false);
		}
		const nextRadius = clampPositive(radius);
		radiusPreviews = { ...radiusPreviews, [endpoint.key]: nextRadius };
		return sendSetParamIntent(
			endpoint.radiusParam.node_id,
			{ kind: 'float', value: nextRadius },
			endpoint.radiusParam.data.param.event_behaviour
		)
			.then((success) => {
				if (!success) {
					clearRadiusPreview(endpoint.key);
				}
				return success;
			})
			.catch(() => {
				clearRadiusPreview(endpoint.key);
				return false;
			});
	};

	const setEndpointFreezeRadius = (endpoint: SpatialEndpoint, radius: number): Promise<boolean> => {
		if (
			endpoint.freezeRadiusParam?.data.kind !== 'parameter' ||
			endpoint.freezeRadiusParam.data.param.read_only
		) {
			return Promise.resolve(false);
		}
		const nextRadius = clampPositive(radius);
		freezeRadiusPreviews = { ...freezeRadiusPreviews, [endpoint.key]: nextRadius };
		return sendSetParamIntent(
			endpoint.freezeRadiusParam.node_id,
			{ kind: 'float', value: nextRadius },
			endpoint.freezeRadiusParam.data.param.event_behaviour
		)
			.then((success) => {
				if (!success) {
					clearFreezeRadiusPreview(endpoint.key);
				}
				return success;
			})
			.catch(() => {
				clearFreezeRadiusPreview(endpoint.key);
				return false;
			});
	};

	const pointerWorld = (event: PointerEvent): Point2 =>
		graphPositionToSpatialPosition(
			graphCanvas?.clientToWorld(event.clientX, event.clientY) ?? { x: 0, y: 0 }
		);

	const dragEditSession = (
		endpoint: SpatialEndpoint,
		kind: 'position' | RadiusDragKind
	): UiEditSession => {
		const verb =
			kind === 'position'
				? 'Move'
				: kind === 'radius'
					? 'Edit radius for'
					: 'Edit freeze radius for';
		return createUiEditSession(`${verb} ${endpoint.node.meta.label}`, 'spatializer-drag');
	};

	const sendDragLatest = (drag: DragGesture): Promise<boolean> => {
		if (drag.kind === 'position') {
			return setEndpointPosition(drag.endpoint, drag.latestPosition);
		}
		if (drag.kind === 'radius') {
			return setEndpointRadius(drag.endpoint, drag.latestRadius);
		}
		return setEndpointFreezeRadius(drag.endpoint, drag.latestRadius);
	};

	const flushDragUpdate = (drag: DragGesture): void => {
		if (drag.sendInFlight || !drag.sendDirty) {
			return;
		}
		drag.sendDirty = false;
		drag.sendInFlight = true;
		drag.updateTail = drag.updateTail
			.then(() => sendDragLatest(drag))
			.then(() => undefined)
			.catch(() => undefined)
			.finally(() => {
				drag.sendInFlight = false;
				if (drag.sendDirty && activeDrag === drag) {
					scheduleDragUpdate(drag);
				}
			});
	};

	const scheduleDragUpdate = (drag: DragGesture): void => {
		drag.sendDirty = true;
		if (drag.sendFrame !== null || drag.sendInFlight) {
			return;
		}
		drag.sendFrame = requestAnimationFrame(() => {
			drag.sendFrame = null;
			flushDragUpdate(drag);
		});
	};

	const drainDragUpdates = async (drag: DragGesture): Promise<void> => {
		if (drag.sendFrame !== null) {
			cancelAnimationFrame(drag.sendFrame);
			drag.sendFrame = null;
		}
		await drag.updateTail;
		if (drag.sendDirty) {
			drag.sendDirty = false;
			await sendDragLatest(drag);
		}
	};

	const startEndpointDrag = (
		event: PointerEvent,
		endpoint: SpatialEndpoint,
		requestedKind: 'position' | RadiusDragKind
	): void => {
		if (event.button !== 0) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		graphCanvas?.focus();
		selectEndpoint(endpoint);
		const pointer = pointerWorld(event);
		const altRadiusKind: RadiusDragKind | null =
			endpoint.radiusWritable && endpoint.radius !== null
				? 'radius'
				: endpoint.freezeRadiusWritable && endpoint.freezeRadius !== null
					? 'freezeRadius'
					: null;
		const radiusKind: RadiusDragKind | null =
			requestedKind === 'position' ? (event.altKey ? altRadiusKind : null) : requestedKind;
		const radius =
			radiusKind === 'radius'
				? endpoint.radius
				: radiusKind === 'freezeRadius'
					? endpoint.freezeRadius
					: null;
		const radiusWritable =
			radiusKind === 'radius'
				? endpoint.radiusWritable
				: radiusKind === 'freezeRadius'
					? endpoint.freezeRadiusWritable
					: false;
		if (radiusKind && radiusWritable && radius !== null) {
			const editSession = dragEditSession(endpoint, radiusKind);
			const updateTail = editSession.begin();
			worldSvg?.setPointerCapture(event.pointerId);
			activeDrag = {
				kind: radiusKind,
				pointerId: event.pointerId,
				endpoint,
				editSession,
				updateTail,
				sendFrame: null,
				sendInFlight: false,
				sendDirty: false,
				latestRadius: radius,
				startRadius: radius,
				startDistance: Math.hypot(pointer.x - endpoint.x, pointer.y - endpoint.y),
				center: { x: endpoint.x, y: endpoint.y }
			};
			return;
		}
		if (!endpoint.positionWritable) {
			return;
		}
		const editSession = dragEditSession(endpoint, 'position');
		const updateTail = editSession.begin();
		worldSvg?.setPointerCapture(event.pointerId);
		activeDrag = {
			kind: 'position',
			pointerId: event.pointerId,
			endpoint,
			editSession,
			updateTail,
			sendFrame: null,
			sendInFlight: false,
			sendDirty: false,
			latestPosition: { x: endpoint.x, y: endpoint.y },
			startPointer: pointer,
			startPosition: { x: endpoint.x, y: endpoint.y }
		};
	};

	const selectEndpointWithKeyboard = (event: KeyboardEvent, endpoint: SpatialEndpoint): void => {
		if (event.key !== 'Enter' && event.key !== ' ') {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		selectEndpoint(endpoint);
	};

	const handleWorldPointerMove = (event: PointerEvent): void => {
		if (!activeDrag || activeDrag.pointerId !== event.pointerId) {
			return;
		}
		event.preventDefault();
		const pointer = pointerWorld(event);
		if (activeDrag.kind === 'position') {
			const drag = activeDrag;
			const nextPosition = {
				x: activeDrag.startPosition.x + pointer.x - activeDrag.startPointer.x,
				y: activeDrag.startPosition.y + pointer.y - activeDrag.startPointer.y
			};
			drag.latestPosition = nextPosition;
			positionPreviews = { ...positionPreviews, [drag.endpoint.key]: nextPosition };
			scheduleDragUpdate(drag);
			return;
		}
		const distance = Math.hypot(pointer.x - activeDrag.center.x, pointer.y - activeDrag.center.y);
		const nextRadius = clampPositive(activeDrag.startRadius + distance - activeDrag.startDistance);
		if (activeDrag.kind === 'radius') {
			const drag = activeDrag;
			drag.latestRadius = nextRadius;
			radiusPreviews = { ...radiusPreviews, [drag.endpoint.key]: nextRadius };
			scheduleDragUpdate(drag);
		} else {
			const drag = activeDrag;
			drag.latestRadius = nextRadius;
			freezeRadiusPreviews = { ...freezeRadiusPreviews, [drag.endpoint.key]: nextRadius };
			scheduleDragUpdate(drag);
		}
	};

	const finishWorldPointer = async (event: PointerEvent): Promise<void> => {
		if (!activeDrag || activeDrag.pointerId !== event.pointerId) {
			return;
		}
		event.preventDefault();
		const drag = activeDrag;
		activeDrag = null;
		if (worldSvg?.hasPointerCapture(event.pointerId)) {
			worldSvg.releasePointerCapture(event.pointerId);
		}
		await drainDragUpdates(drag);
		await drag.editSession.end();
	};

	const cancelWorldPointer = async (event: PointerEvent): Promise<void> => {
		if (!activeDrag || activeDrag.pointerId !== event.pointerId) {
			return;
		}
		const drag = activeDrag;
		const key = drag.endpoint.key;
		if (drag.kind === 'position') {
			clearPositionPreview(key);
		} else if (drag.kind === 'radius') {
			clearRadiusPreview(key);
		} else {
			clearFreezeRadiusPreview(key);
		}
		activeDrag = null;
		await drainDragUpdates(drag);
		await drag.editSession.end();
	};

	return {
		get positionPreviews() {
			return positionPreviews;
		},
		get radiusPreviews() {
			return radiusPreviews;
		},
		get freezeRadiusPreviews() {
			return freezeRadiusPreviews;
		},
		startEndpointDrag,
		selectEndpointWithKeyboard,
		handleWorldPointerMove,
		finishWorldPointer,
		cancelWorldPointer
	};
}
export type SpatializerInteraction = ReturnType<typeof createSpatializerInteraction>;
