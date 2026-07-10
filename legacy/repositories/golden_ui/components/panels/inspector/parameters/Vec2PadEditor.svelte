<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { registerCommandHandler } from '../../../../store/commands.svelte';
	import { appState } from '../../../../store/workbench.svelte';
	import { createUiEditSession, sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto, UiRangeConstraint } from '../../../../types';
	import { formatWatcherNumber } from '../../../common/watcher/watcher-utils';

	interface Bounds {
		xMin: number;
		xMax: number;
		yMin: number;
		yMax: number;
	}

	interface GridLine {
		value: number;
		position: number;
		label: string;
	}

	interface TrailPoint {
		x: number;
		y: number;
		timestampMs: number;
	}

	interface TrailSegment {
		x1: number;
		y1: number;
		x2: number;
		y2: number;
		opacity: number;
	}

	type DragState =
		| {
				kind: 'handle';
				pointerId: number;
				startClientX: number;
				startClientY: number;
				startValue: [number, number];
				bounds: Bounds;
		  }
		| {
				kind: 'pan';
				pointerId: number;
				startClientX: number;
				startClientY: number;
				startCameraX: number;
				startCameraY: number;
				bounds: Bounds;
		  };

	let {
		node,
		layoutMode = 'default',
		rangeOverride = null,
		trailSeconds = undefined
	} = $props<{
		node: UiNodeDto;
		layoutMode?: 'default' | 'widget';
		rangeOverride?: UiRangeConstraint | null;
		trailSeconds?: number | null;
	}>();

	const EPSILON = 1e-9;
	const TRAIL_REFRESH_MS = 48;
	const TRAIL_SAMPLE_INTERVAL_MS = 1000 / 90;
	const TRAIL_POINT_CAP_MAX = 8192;
	const DEFAULT_TRAIL_SECONDS = 2;
	const DEFAULT_UNIT_STEP = 1;
	const DEFAULT_VIEW_SPAN = 8;
	const FULL_PERCENT = 100;

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let readOnly = $derived(Boolean(param?.read_only));
	let enabled = $derived(liveNode.meta.enabled);
	let unitSuffix = $derived.by((): string => {
		const rawUnit = param?.ui_hints.unit?.trim() ?? '';
		return rawUnit.length > 0 ? ` ${rawUnit}` : '';
	});
	let currentValue = $derived.by((): [number, number] => {
		if (!param || param.value.kind !== 'vec2') {
			return [0, 0];
		}
		return [Number(param.value.value[0] ?? 0), Number(param.value.value[1] ?? 0)];
	});
	let rangedBounds = $derived(parseVec2Bounds(rangeOverride ?? param?.constraints.range));
	let isRanged = $derived(rangedBounds !== null);
	let safeViewSpan = $derived(Math.max(0.25, DEFAULT_VIEW_SPAN));
	let plotAspectRatio = $derived.by((): number => {
		if (!rangedBounds) {
			if (layoutMode === 'widget') {
				return shellAspectRatio;
			}
			return 1;
		}
		const width = Math.max(EPSILON, rangedBounds.xMax - rangedBounds.xMin);
		const height = Math.max(EPSILON, rangedBounds.yMax - rangedBounds.yMin);
		return width / height;
	});
	let effectivePlotAspectRatio = $derived(Math.max(EPSILON, plotAspectRatio));

	let shellElement = $state<HTMLDivElement | null>(null);
	let stageElement = $state<HTMLDivElement | null>(null);
	let shellWidth = $state(0);
	let shellHeight = $state(0);
	let cameraCenterX = $state(0);
	let cameraCenterY = $state(0);
	let cameraHeightSpan = $state(DEFAULT_VIEW_SPAN);
	let draftValue = $state<[number, number]>([0, 0]);
	let tickNowMs = $state(Date.now());
	let dragState = $state<DragState | null>(null);
	let trailPoints = $state<TrailPoint[]>([]);
	let lastTrailSignature = $state('');
	let isStageFocused = $state(false);
	let editSession = createUiEditSession('Edit vec2 in 2D', 'param-vec2-2d');

	const clampPercent = (value: number): number =>
		Math.max(0, Math.min(FULL_PERCENT, value * FULL_PERCENT));

	let shellAspectRatio = $derived.by((): number => {
		if (shellWidth <= EPSILON || shellHeight <= EPSILON) {
			return 1;
		}
		return shellWidth / shellHeight;
	});
	let plotViewWidth = $derived.by((): number =>
		effectivePlotAspectRatio >= 1 ? FULL_PERCENT * effectivePlotAspectRatio : FULL_PERCENT
	);
	let plotViewHeight = $derived.by((): number =>
		effectivePlotAspectRatio >= 1 ? FULL_PERCENT : FULL_PERCENT / effectivePlotAspectRatio
	);
	let plotViewBox = $derived(`0 0 ${plotViewWidth} ${plotViewHeight}`);
	let effectiveTrailSeconds = $derived.by((): number => {
		if (trailSeconds === null) {
			return 0;
		}
		if (trailSeconds === undefined) {
			return DEFAULT_TRAIL_SECONDS;
		}
		if (!Number.isFinite(trailSeconds)) {
			return DEFAULT_TRAIL_SECONDS;
		}
		return Math.max(0, trailSeconds);
	});
	let showsTrail = $derived(effectiveTrailSeconds > 0);
	let maxTrailPointCount = $derived.by((): number => {
		if (!showsTrail) {
			return 0;
		}
		return Math.max(
			2,
			Math.min(
				TRAIL_POINT_CAP_MAX,
				Math.ceil((effectiveTrailSeconds * 1000) / TRAIL_SAMPLE_INTERVAL_MS) + 2
			)
		);
	});
	let stageInlineFit = $derived.by((): string => {
		if (layoutMode !== 'widget') {
			return '100%';
		}
		if (effectivePlotAspectRatio >= shellAspectRatio) {
			return '100%';
		}
		return `${clampPercent(effectivePlotAspectRatio / Math.max(EPSILON, shellAspectRatio))}%`;
	});
	let stageBlockFit = $derived.by((): string => {
		if (layoutMode !== 'widget') {
			return 'auto';
		}
		if (effectivePlotAspectRatio >= shellAspectRatio) {
			return `${clampPercent(shellAspectRatio / effectivePlotAspectRatio)}%`;
		}
		return '100%';
	});

	function normalizeBounds(bounds: Bounds): Bounds {
		let xMin = bounds.xMin;
		let xMax = bounds.xMax;
		let yMin = bounds.yMin;
		let yMax = bounds.yMax;

		if (Math.abs(xMax - xMin) <= EPSILON) {
			const center = (xMin + xMax) * 0.5;
			xMin = center - 0.5;
			xMax = center + 0.5;
		}

		if (Math.abs(yMax - yMin) <= EPSILON) {
			const center = (yMin + yMax) * 0.5;
			yMin = center - 0.5;
			yMax = center + 0.5;
		}

		return { xMin, xMax, yMin, yMax };
	}

	function parseVec2Bounds(range: UiRangeConstraint | undefined): Bounds | null {
		if (!range) {
			return null;
		}

		if (range.kind === 'uniform') {
			if (
				!Number.isFinite(range.min) ||
				!Number.isFinite(range.max) ||
				range.min === undefined ||
				range.max === undefined
			) {
				return null;
			}
			if (range.min > range.max) {
				return null;
			}
			return normalizeBounds({
				xMin: range.min,
				xMax: range.max,
				yMin: range.min,
				yMax: range.max
			});
		}

		const xMin = range.min?.[0];
		const yMin = range.min?.[1];
		const xMax = range.max?.[0];
		const yMax = range.max?.[1];
		if (
			xMin === undefined ||
			yMin === undefined ||
			xMax === undefined ||
			yMax === undefined ||
			!Number.isFinite(xMin) ||
			!Number.isFinite(yMin) ||
			!Number.isFinite(xMax) ||
			!Number.isFinite(yMax) ||
			xMin > xMax ||
			yMin > yMax
		) {
			return null;
		}

		return normalizeBounds({ xMin, xMax, yMin, yMax });
	}

	const visibleBounds = $derived.by((): Bounds => {
		const halfHeight = Math.max(EPSILON, cameraHeightSpan) * 0.5;
		const halfWidth = halfHeight * effectivePlotAspectRatio;
		return {
			xMin: cameraCenterX - halfWidth,
			xMax: cameraCenterX + halfWidth,
			yMin: cameraCenterY - halfHeight,
			yMax: cameraCenterY + halfHeight
		};
	});

	const clampPointToBounds = (point: [number, number], bounds: Bounds | null): [number, number] => {
		if (!bounds) {
			return point;
		}
		return [
			Math.min(bounds.xMax, Math.max(bounds.xMin, point[0])),
			Math.min(bounds.yMax, Math.max(bounds.yMin, point[1]))
		];
	};

	const toPlotX = (value: number, bounds: Bounds): number =>
		((value - bounds.xMin) / Math.max(EPSILON, bounds.xMax - bounds.xMin)) * plotViewWidth;
	const toPlotY = (value: number, bounds: Bounds): number =>
		plotViewHeight -
		((value - bounds.yMin) / Math.max(EPSILON, bounds.yMax - bounds.yMin)) * plotViewHeight;

	const computeGridStep = (baseStep: number, span: number, targetLineCount = 8): number => {
		const safeBase = Math.max(0.0001, Math.abs(baseStep));
		const roughStep = Math.max(safeBase, span / Math.max(2, targetLineCount));
		const exponent = Math.floor(Math.log10(roughStep / safeBase));
		const scaledBase = safeBase * Math.pow(10, exponent);

		for (const factor of [1, 2, 5, 10]) {
			const candidate = scaledBase * factor;
			if (candidate >= roughStep - EPSILON) {
				return candidate;
			}
		}

		return scaledBase * 10;
	};

	const buildGridLines = (
		minValue: number,
		maxValue: number,
		step: number,
		mapValueToPosition: (value: number) => number
	): GridLine[] => {
		const safeStep = Math.max(0.0001, step);
		const first = Math.ceil(minValue / safeStep) * safeStep;
		const maxLines = 48;
		const lines: GridLine[] = [];

		let lineCount = 0;
		for (
			let value = first;
			value <= maxValue + safeStep * 0.001 && lineCount < maxLines;
			value += safeStep, lineCount += 1
		) {
			lines.push({
				value,
				position: mapValueToPosition(value),
				label: formatWatcherNumber(value)
			});
		}

		return lines;
	};

	const xGridLines = $derived.by((): GridLine[] => {
		const bounds = visibleBounds;
		const step = computeGridStep(DEFAULT_UNIT_STEP, bounds.xMax - bounds.xMin, 8);
		return buildGridLines(bounds.xMin, bounds.xMax, step, (value) => toPlotX(value, bounds));
	});

	const yGridLines = $derived.by((): GridLine[] => {
		const bounds = visibleBounds;
		const step = computeGridStep(DEFAULT_UNIT_STEP, bounds.yMax - bounds.yMin, 8);
		return buildGridLines(bounds.yMin, bounds.yMax, step, (value) => toPlotY(value, bounds));
	});

	const labelStride = (lineCount: number): number => Math.max(1, Math.ceil(lineCount / 8));

	const prunedTrailPoints = $derived.by((): TrailPoint[] => {
		const maxAgeMs = effectiveTrailSeconds * 1000;
		if (maxAgeMs <= 0) {
			return [];
		}
		return trailPoints.filter((point) => tickNowMs - point.timestampMs <= maxAgeMs);
	});

	const trailSegments = $derived.by((): TrailSegment[] => {
		const points = prunedTrailPoints;
		const bounds = visibleBounds;
		const trailAgeMs = effectiveTrailSeconds * 1000;
		if (points.length < 2 || trailAgeMs <= 0) {
			return [];
		}

		const segments: TrailSegment[] = [];
		for (let index = 1; index < points.length; index += 1) {
			const start = points[index - 1];
			const end = points[index];
			const ageMs = tickNowMs - (start.timestampMs + end.timestampMs) * 0.5;
			const opacity = Math.max(0.08, 1 - ageMs / trailAgeMs);
			segments.push({
				x1: toPlotX(start.x, bounds),
				y1: toPlotY(start.y, bounds),
				x2: toPlotX(end.x, bounds),
				y2: toPlotY(end.y, bounds),
				opacity
			});
		}

		return segments;
	});

	const displayedValue = $derived.by((): [number, number] =>
		clampPointToBounds(draftValue, rangedBounds)
	);
	const handleX = $derived.by(() => toPlotX(displayedValue[0], visibleBounds));
	const handleY = $derived.by(() => toPlotY(displayedValue[1], visibleBounds));
	const zeroAxisX = $derived.by(() =>
		visibleBounds.xMin < 0 && visibleBounds.xMax > 0 ? toPlotX(0, visibleBounds) : null
	);
	const zeroAxisY = $derived.by(() =>
		visibleBounds.yMin < 0 && visibleBounds.yMax > 0 ? toPlotY(0, visibleBounds) : null
	);
	const isHandleDragging = $derived(dragState?.kind === 'handle');
	const isPanning = $derived(dragState?.kind === 'pan');

	const updateDraftValue = (nextValue: [number, number]): void => {
		draftValue = clampPointToBounds(nextValue, rangedBounds);
	};

	const commitValue = (nextValue: [number, number]): void => {
		if (!param || param.value.kind !== 'vec2' || readOnly || !enabled) {
			return;
		}

		const normalized = clampPointToBounds(nextValue, rangedBounds);
		updateDraftValue(normalized);

		if (
			Math.abs(normalized[0] - currentValue[0]) <= EPSILON &&
			Math.abs(normalized[1] - currentValue[1]) <= EPSILON
		) {
			return;
		}

		void sendSetParamIntent(
			liveNode.node_id,
			{ kind: 'vec2', value: normalized },
			param.event_behaviour
		);
	};

	const focusStage = (): void => {
		stageElement?.focus();
	};

	const resetCamera = (): void => {
		if (rangedBounds) {
			cameraCenterX = (rangedBounds.xMin + rangedBounds.xMax) * 0.5;
			cameraCenterY = (rangedBounds.yMin + rangedBounds.yMax) * 0.5;
			cameraHeightSpan = Math.max(EPSILON, rangedBounds.yMax - rangedBounds.yMin);
			return;
		}
		const [x, y] = displayedValue;
		cameraCenterX = x;
		cameraCenterY = y;
		cameraHeightSpan = safeViewSpan;
	};

	const stopInteraction = (pointerId: number | null = null): void => {
		if (pointerId !== null && stageElement?.hasPointerCapture(pointerId)) {
			stageElement.releasePointerCapture(pointerId);
		}
		if (dragState?.kind === 'handle') {
			void editSession.end();
		}
		dragState = null;
	};

	const beginHandleDrag = (event: PointerEvent): void => {
		if (!stageElement || readOnly || !enabled) {
			return;
		}

		event.preventDefault();
		dragState = {
			kind: 'handle',
			pointerId: event.pointerId,
			startClientX: event.clientX,
			startClientY: event.clientY,
			startValue: [...displayedValue] as [number, number],
			bounds: visibleBounds
		};
		stageElement.setPointerCapture(event.pointerId);
		void editSession.begin();
	};

	const beginPan = (event: PointerEvent): void => {
		if (!stageElement || !isRanged) {
			return;
		}

		event.preventDefault();
		dragState = {
			kind: 'pan',
			pointerId: event.pointerId,
			startClientX: event.clientX,
			startClientY: event.clientY,
			startCameraX: cameraCenterX,
			startCameraY: cameraCenterY,
			bounds: visibleBounds
		};
		stageElement.setPointerCapture(event.pointerId);
	};

	const handlePointerDown = (event: PointerEvent): void => {
		if (!param || param.value.kind !== 'vec2') {
			return;
		}

		focusStage();

		if (isRanged && (event.button === 1 || event.button === 2)) {
			event.stopPropagation();
			beginPan(event);
			return;
		}

		if (event.button === 0) {
			event.stopPropagation();
			beginHandleDrag(event);
		}
	};

	const handlePointerMove = (event: PointerEvent): void => {
		if (!dragState || !stageElement || dragState.pointerId !== event.pointerId) {
			return;
		}

		const rect = stageElement.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) {
			return;
		}

		const deltaX = event.clientX - dragState.startClientX;
		const deltaY = event.clientY - dragState.startClientY;
		const spanX = dragState.bounds.xMax - dragState.bounds.xMin;
		const spanY = dragState.bounds.yMax - dragState.bounds.yMin;

		if (dragState.kind === 'handle') {
			const nextX = dragState.startValue[0] + (deltaX / rect.width) * spanX;
			const nextY = dragState.startValue[1] - (deltaY / rect.height) * spanY;
			commitValue([nextX, nextY]);
			return;
		}

		cameraCenterX = dragState.startCameraX - (deltaX / rect.width) * spanX;
		cameraCenterY = dragState.startCameraY + (deltaY / rect.height) * spanY;
	};

	const handlePointerUp = (event: PointerEvent): void => {
		if (!dragState || dragState.pointerId !== event.pointerId) {
			return;
		}
		stopInteraction(event.pointerId);
	};

	const handleWheel = (event: WheelEvent): void => {
		if (!stageElement) {
			return;
		}

		focusStage();
		event.preventDefault();
		event.stopPropagation();

		const rect = stageElement.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) {
			return;
		}

		const bounds = visibleBounds;
		const localX = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
		const localY = Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height));
		const anchorX = bounds.xMin + localX * (bounds.xMax - bounds.xMin);
		const anchorY = bounds.yMax - localY * (bounds.yMax - bounds.yMin);

		const zoomFactor = Math.exp(event.deltaY * 0.0015);
		const minSpan = Math.max(0.05, DEFAULT_UNIT_STEP * 0.25);
		const maxSpan = Math.max(minSpan, 1000000);
		const nextHeightSpan = Math.min(maxSpan, Math.max(minSpan, cameraHeightSpan * zoomFactor));

		if (!isRanged) {
			cameraHeightSpan = nextHeightSpan;
			const [x, y] = displayedValue;
			cameraCenterX = x;
			cameraCenterY = y;
			return;
		}

		const nextWidthSpan = nextHeightSpan * effectivePlotAspectRatio;
		const nextXMin = anchorX - localX * nextWidthSpan;
		const nextYMax = anchorY + localY * nextHeightSpan;
		cameraHeightSpan = nextHeightSpan;
		cameraCenterX = nextXMin + nextWidthSpan * 0.5;
		cameraCenterY = nextYMax - nextHeightSpan * 0.5;
	};

	const handleDoubleClick = (event: MouseEvent): void => {
		focusStage();
		event.preventDefault();
		event.stopPropagation();
		resetCamera();
	};

	$effect(() => {
		const element = shellElement;
		if (!element) {
			return;
		}

		const syncShellSize = (): void => {
			const rect = element.getBoundingClientRect();
			shellWidth = Math.max(0, rect.width);
			shellHeight = Math.max(0, rect.height);
		};

		syncShellSize();

		let resizeFrameId: number | null = null;
		const scheduleShellSizeSync = (): void => {
			if (typeof window === 'undefined') {
				syncShellSize();
				return;
			}
			if (resizeFrameId !== null) {
				return;
			}
			resizeFrameId = window.requestAnimationFrame(() => {
				resizeFrameId = null;
				syncShellSize();
			});
		};

		if (typeof ResizeObserver === 'undefined') {
			return () => {
				if (resizeFrameId !== null && typeof window !== 'undefined') {
					window.cancelAnimationFrame(resizeFrameId);
				}
			};
		}

		const resizeObserver = new ResizeObserver(() => {
			scheduleShellSizeSync();
		});
		resizeObserver.observe(element);

		return () => {
			if (resizeFrameId !== null && typeof window !== 'undefined') {
				window.cancelAnimationFrame(resizeFrameId);
			}
			resizeObserver.disconnect();
		};
	});

	$effect(() => {
		liveNode.node_id;
		untrack(() => {
			lastTrailSignature = '';
			trailPoints = [];
			draftValue = [...currentValue] as [number, number];
			resetCamera();
		});
	});

	$effect(() => {
		if (isHandleDragging) {
			return;
		}
		draftValue = [...currentValue] as [number, number];
	});

	$effect(() => {
		if (isRanged || isPanning) {
			return;
		}
		const [x, y] = displayedValue;
		cameraCenterX = x;
		cameraCenterY = y;
	});

	$effect(() => {
		const unregisterHandlers = [
			registerCommandHandler(
				'view.frame',
				() => {
					if (!isStageFocused) {
						return false;
					}
					resetCamera();
					return true;
				},
				{ priority: 300 }
			),
			registerCommandHandler(
				'view.home',
				() => {
					if (!isStageFocused) {
						return false;
					}
					resetCamera();
					return true;
				},
				{ priority: 300 }
			)
		];

		return () => {
			for (const unregister of unregisterHandlers) {
				unregister();
			}
		};
	});

	$effect(() => {
		if (showsTrail) {
			return;
		}
		lastTrailSignature = '';
		trailPoints = [];
	});

	$effect(() => {
		if (!param || param.value.kind !== 'vec2') {
			return;
		}
		if (!showsTrail) {
			return;
		}

		const [x, y] = displayedValue;
		const signature = `${x}|${y}`;
		if (signature === lastTrailSignature) {
			return;
		}

		lastTrailSignature = signature;
		const nextPoint = {
			x,
			y,
			timestampMs: Date.now()
		};
		const lastPoint = trailPoints.at(-1);
		if (lastPoint && nextPoint.timestampMs - lastPoint.timestampMs < TRAIL_SAMPLE_INTERVAL_MS) {
			trailPoints = [...trailPoints.slice(0, -1), nextPoint];
			return;
		}

		const nextTrail = [...trailPoints, nextPoint];
		const overflow = Math.max(0, nextTrail.length - maxTrailPointCount);
		trailPoints = overflow > 0 ? nextTrail.slice(overflow) : nextTrail;
	});

	$effect(() => {
		if (typeof window === 'undefined') {
			return;
		}

		const intervalId = window.setInterval(() => {
			tickNowMs = Date.now();
		}, TRAIL_REFRESH_MS);

		return () => {
			window.clearInterval(intervalId);
		};
	});

	onDestroy(() => {
		stopInteraction();
		void editSession.end();
	});
</script>

<div class="vec2-pad-editor" class:widget-layout={layoutMode === 'widget'}>
	<div bind:this={shellElement} class="vec2-pad-shell">
		<div
			bind:this={stageElement}
			class="vec2-pad-stage"
			class:dragging={isHandleDragging}
			class:panning={isPanning}
			role="application"
			tabindex="-1"
			aria-label="2D vec2 editor"
			style={`--gc-vec2-pad-aspect:${plotAspectRatio};--gc-vec2-pad-inline-fit:${stageInlineFit};--gc-vec2-pad-block-fit:${stageBlockFit};`}
			onfocusin={() => {
				isStageFocused = true;
			}}
			onfocusout={(event) => {
				const nextTarget = event.relatedTarget;
				if (
					nextTarget instanceof Node &&
					(event.currentTarget as HTMLElement).contains(nextTarget)
				) {
					return;
				}
				isStageFocused = false;
			}}
			oncontextmenu={(event) => {
				event.preventDefault();
			}}
			ondblclick={handleDoubleClick}
			onpointerdown={handlePointerDown}
			onpointermove={handlePointerMove}
			onpointerup={handlePointerUp}
			onpointercancel={handlePointerUp}
			onlostpointercapture={() => {
				stopInteraction();
			}}
			onwheel={handleWheel}>
			<svg
				class="vec2-pad-svg"
				viewBox={plotViewBox}
				preserveAspectRatio="xMidYMid meet"
				aria-hidden="true">
				<rect class="vec2-pad-background" x="0" y="0" width={plotViewWidth} height={plotViewHeight}
				></rect>

				{#each yGridLines as line}
					<line
						class="vec2-pad-grid"
						x1="0"
						y1={line.position}
						x2={plotViewWidth}
						y2={line.position}></line>
				{/each}
				{#each xGridLines as line}
					<line
						class="vec2-pad-grid"
						x1={line.position}
						y1="0"
						x2={line.position}
						y2={plotViewHeight}></line>
				{/each}

				{#if zeroAxisX !== null}
					<line class="vec2-pad-axis" x1={zeroAxisX} y1="0" x2={zeroAxisX} y2={plotViewHeight}
					></line>
				{/if}
				{#if zeroAxisY !== null}
					<line class="vec2-pad-axis" x1="0" y1={zeroAxisY} x2={plotViewWidth} y2={zeroAxisY}
					></line>
				{/if}

				<g>
					{#each trailSegments as segment}
						<line
							class="vec2-pad-trail"
							x1={segment.x1}
							y1={segment.y1}
							x2={segment.x2}
							y2={segment.y2}
							style={`opacity:${segment.opacity};stroke-width:${0.45 + segment.opacity * 0.65};`}
						></line>
					{/each}

					<line class="vec2-pad-crosshair" x1={handleX} y1="0" x2={handleX} y2={plotViewHeight}
					></line>
					<line class="vec2-pad-crosshair" x1="0" y1={handleY} x2={plotViewWidth} y2={handleY}
					></line>
				</g>

				{#each xGridLines as line, index}
					{#if index % labelStride(xGridLines.length) === 0}
						<text class="vec2-pad-label" x={line.position + 0.8} y={plotViewHeight - 3.5}>
							{line.label}
						</text>
					{/if}
				{/each}

				{#each yGridLines as line, index}
					{#if index % labelStride(yGridLines.length) === 0}
						<text
							class="vec2-pad-label"
							x="1.3"
							y={Math.max(4.5, Math.min(plotViewHeight - 3.5, line.position - 0.8))}>
							{line.label}
						</text>
					{/if}
				{/each}

				<circle class="vec2-pad-handle-glow" cx={handleX} cy={handleY} r="3.6"></circle>
				<circle class="vec2-pad-handle" cx={handleX} cy={handleY} r="1.7"></circle>
			</svg>

			<div class="vec2-pad-overlay top-left">
				<span>X {formatWatcherNumber(draftValue[0])}{unitSuffix}</span>
				<span>Y {formatWatcherNumber(draftValue[1])}{unitSuffix}</span>
				<span>{isRanged ? 'bounded' : 'free'}</span>
			</div>

			{#if !isRanged}
				<div class="vec2-pad-overlay bottom-right">wheel zoom | double-click reset</div>
			{:else}
				<div class="vec2-pad-overlay bottom-right">
					wheel zoom | right-drag pan | double-click reset
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.vec2-pad-editor {
		display: flex;
		inline-size: 100%;
		min-inline-size: 0;
	}

	.vec2-pad-shell {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		justify-content: center;
		min-inline-size: 0;
		min-block-size: 11rem;
	}

	.vec2-pad-stage {
		position: relative;
		inline-size: var(--gc-vec2-pad-inline-fit, 100%);
		block-size: var(--gc-vec2-pad-block-fit, auto);
		max-inline-size: 100%;
		min-inline-size: 0;
		aspect-ratio: var(--gc-vec2-pad-aspect);
		border-radius: 0.7rem;
		overflow: hidden;
		user-select: none;
		touch-action: none;
		cursor: crosshair;
		background:
			radial-gradient(
				circle at 20% 20%,
				rgb(from var(--gc-color-selection) r g b / 18%),
				transparent 55%
			),
			linear-gradient(
				180deg,
				rgb(from var(--gc-color-slider-bg) r g b / 86%),
				rgb(from var(--gc-color-slider-bg) r g b / 60%)
			);
		box-shadow:
			inset 0 0 0 0.08rem rgb(from var(--gc-color-selection) r g b / 22%),
			inset 0 0 0 0.14rem rgb(from var(--gc-color-text) r g b / 8%);
	}

	.vec2-pad-stage.dragging {
		cursor: none;
	}

	.vec2-pad-stage.panning {
		cursor: grabbing;
	}

	.widget-layout .vec2-pad-shell {
		block-size: 100%;
		min-block-size: 0;
	}

	.widget-layout .vec2-pad-stage {
		flex: 0 1 auto;
		max-inline-size: 100%;
		max-block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.vec2-pad-svg {
		display: block;
		inline-size: 100%;
		block-size: 100%;
	}

	.vec2-pad-background {
		fill: rgb(from var(--gc-color-slider-bg) r g b / 12%);
	}

	.vec2-pad-grid {
		stroke: rgb(from var(--gc-color-text) r g b / 11%);
		stroke-width: 0.22;
	}

	.vec2-pad-axis {
		stroke: rgb(from var(--gc-color-text) r g b / 30%);
		stroke-width: 0.45;
	}

	.vec2-pad-trail {
		stroke: color-mix(in srgb, var(--gc-color-selection) 74%, white 26%);
		stroke-linecap: round;
	}

	.vec2-pad-crosshair {
		stroke: rgb(from var(--gc-color-selection) r g b / 28%);
		stroke-width: 0.26;
		stroke-dasharray: 1.2 1.2;
	}

	.vec2-pad-handle-glow {
		fill: rgb(from var(--gc-color-selection) r g b / 22%);
	}

	.vec2-pad-handle {
		fill: color-mix(in srgb, var(--gc-color-selection) 72%, white 28%);
		stroke: rgb(from var(--gc-color-text) r g b / 66%);
		stroke-width: 0.38;
	}

	.vec2-pad-label {
		fill: rgb(from var(--gc-color-text) r g b / 58%);
		font-size: 3.4px;
		paint-order: stroke;
		stroke: rgb(from var(--gc-color-slider-bg) r g b / 75%);
		stroke-width: 0.55;
	}

	.vec2-pad-overlay {
		position: absolute;
		display: inline-flex;
		flex-wrap: wrap;
		gap: 0.35rem;
		max-inline-size: calc(100% - 1.2rem);
		padding: 0.2rem 0.4rem;
		border-radius: 999rem;
		background: rgb(from var(--gc-color-slider-bg) r g b / 66%);
		backdrop-filter: blur(0.35rem);
		font-size: 0.62rem;
		line-height: 1.2;
		color: rgb(from var(--gc-color-text) r g b / 84%);
	}

	.vec2-pad-overlay.top-left {
		inset-block-start: 0.45rem;
		inset-inline-start: 0.45rem;
	}

	.vec2-pad-overlay.bottom-right {
		inset-block-end: 0.45rem;
		inset-inline-end: 0.45rem;
		justify-content: flex-end;
		text-align: right;
	}
</style>
