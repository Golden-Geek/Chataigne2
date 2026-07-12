<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import {
		sendCreateUserItemByTypeIntent,
		sendRemoveNodeIntent,
		sendSetParamIntent,
		createUiEditSession
	} from '../../../../store/ui-intents';
	import { registerCommandHandler } from '../../../../store/commands.svelte';
	import type { NodeId, ParamValue, UiNodeDto } from '../../../../types';
	import type { NodeInspectorComponentProps } from '../node-inspector-registry';
	import NodeInspector from '../NodeInspector.svelte';
	import SelectNodeButton from '../../../common/SelectNodeButton.svelte';
	import ColorPicker from '../../../common/ColorPicker.svelte';
	import { ColorUtil, type Color } from '../../../common/Color.svelte';

	let { node, level, defaultHeader, defaultContent }: NodeInspectorComponentProps = $props();

	const GRADIENT_NODE_TYPE = 'gradient';
	const STOP_NODE_TYPE = 'gradient_stop';
	const STOP_ITEM_TYPE = 'gradient_stop';
	const DECL_POSITION = 'position';
	const DECL_COLOR = 'color';
	const DECL_INTERPOLATION = 'interpolation';
	const INTERPOLATIONS = ['none', 'linear', 'smooth'] as const;
	const SMOOTH_STEPS = 8;
	const DRAG_THRESHOLD_PX = 2;
	const REMOVE_DRAG_PX = 26;

	type Rgba = [number, number, number, number];
	type StopData = {
		id: NodeId;
		position: number;
		color: Rgba;
		interpolation: string;
		positionParamId: NodeId | null;
		colorParamId: NodeId | null;
		interpolationParamId: NodeId | null;
	};

	let session = $derived(appState.session);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);
	let liveNode: UiNodeDto = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);

	let selected_stop_id = $state<NodeId | null>(null);
	let color_editor_stop_id = $state<NodeId | null>(null);
	let picker_color = $state<Color>({ r: 0, g: 0, b: 0, a: 1 });

	let editorEl: HTMLDivElement | null = $state(null);
	let is_editor_focused = $state(false);
	let trackEl: HTMLDivElement | null = $state(null);
	let dragStopId: NodeId | null = null;
	let dragPointerId: number | null = null;
	let dragStartX = 0;
	let dragStartY = 0;
	let dragMoved = false;
	let dragSession: ReturnType<typeof createUiEditSession> | null = null;
	let remove_armed_stop_id = $state<NodeId | null>(null);
	let color_session: ReturnType<typeof createUiEditSession> | null = null;

	const clamp01 = (value: number): number => Math.min(1, Math.max(0, value));

	const childParam = (parent: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!graphNodesById) {
			return null;
		}
		for (const childId of parent.children) {
			const child = graphNodesById.get(childId);
			if (child?.decl_id === declId && child.data.kind === 'parameter') {
				return child;
			}
		}
		return null;
	};

	let stops = $derived.by((): StopData[] => {
		if (!graphNodesById || liveNode.node_type !== GRADIENT_NODE_TYPE) {
			return [];
		}
		const result: StopData[] = [];
		for (const childId of liveNode.children) {
			const child = graphNodesById.get(childId);
			if (!child || child.node_type !== STOP_NODE_TYPE) {
				continue;
			}
			const positionNode = childParam(child, DECL_POSITION);
			const colorNode = childParam(child, DECL_COLOR);
			const interpolationNode = childParam(child, DECL_INTERPOLATION);
			const positionValue =
				positionNode?.data.kind === 'parameter' ? positionNode.data.param.value : null;
			const colorValue = colorNode?.data.kind === 'parameter' ? colorNode.data.param.value : null;
			const interpolationValue =
				interpolationNode?.data.kind === 'parameter' ? interpolationNode.data.param.value : null;
			const position =
				positionValue?.kind === 'float' || positionValue?.kind === 'int' ? positionValue.value : 0;
			const color = (colorValue?.kind === 'color' ? colorValue.value : [0, 0, 0, 1]) as Rgba;
			const interpolation =
				interpolationValue?.kind === 'enum' ? interpolationValue.value : 'linear';
			result.push({
				id: child.node_id,
				position,
				color,
				interpolation,
				positionParamId: positionNode?.node_id ?? null,
				colorParamId: colorNode?.node_id ?? null,
				interpolationParamId: interpolationNode?.node_id ?? null
			});
		}
		result.sort((left, right) => left.position - right.position);
		return result;
	});

	let selected_stop_node = $derived.by((): UiNodeDto | null => {
		if (selected_stop_id === null) {
			return null;
		}
		return graphNodesById?.get(selected_stop_id) ?? null;
	});

	let stop_count_label = $derived(`${stops.length} stop${stops.length === 1 ? '' : 's'}`);

	const cssColor = (color: Rgba): string => {
		const r = Math.round(clamp01(color[0]) * 255);
		const g = Math.round(clamp01(color[1]) * 255);
		const b = Math.round(clamp01(color[2]) * 255);
		return `rgb(${r} ${g} ${b} / ${clamp01(color[3])})`;
	};

	const lerp = (start: number, end: number, amount: number): number =>
		start + (end - start) * amount;
	const lerpColor = (start: Rgba, end: Rgba, amount: number): Rgba => [
		lerp(start[0], end[0], amount),
		lerp(start[1], end[1], amount),
		lerp(start[2], end[2], amount),
		lerp(start[3], end[3], amount)
	];
	const applyInterpolation = (interpolation: string, amount: number): number => {
		const t = clamp01(amount);
		if (interpolation === 'none') {
			return 0;
		}
		if (interpolation === 'smooth') {
			return t * t * (3 - 2 * t);
		}
		return t;
	};

	const sampleColorAt = (position: number): Rgba => {
		if (stops.length === 0) {
			return [0, 0, 0, 1];
		}
		if (position <= stops[0].position) {
			return stops[0].color;
		}
		const last = stops[stops.length - 1];
		if (position >= last.position) {
			return last.color;
		}
		for (let index = 1; index < stops.length; index += 1) {
			const left = stops[index - 1];
			const right = stops[index];
			if (position > right.position) {
				continue;
			}
			const span = Math.max(1e-9, right.position - left.position);
			const amount = applyInterpolation(left.interpolation, (position - left.position) / span);
			return lerpColor(left.color, right.color, amount);
		}
		return last.color;
	};

	// Interpolation-aware CSS preview: hard edges for `none`, sampled steps for `smooth`.
	let gradientCss = $derived.by((): string => {
		if (stops.length === 0) {
			return 'linear-gradient(to right, #000, #000)';
		}
		if (stops.length === 1) {
			const single = cssColor(stops[0].color);
			return `linear-gradient(to right, ${single}, ${single})`;
		}
		const percent = (value: number): string => `${(clamp01(value) * 100).toFixed(2)}%`;
		const parts: string[] = [`${cssColor(stops[0].color)} ${percent(stops[0].position)}`];
		for (let index = 1; index < stops.length; index += 1) {
			const left = stops[index - 1];
			const right = stops[index];
			if (left.interpolation === 'none') {
				parts.push(`${cssColor(left.color)} ${percent(right.position)}`);
			} else if (left.interpolation === 'smooth') {
				for (let step = 1; step < SMOOTH_STEPS; step += 1) {
					const t = step / SMOOTH_STEPS;
					const position = lerp(left.position, right.position, t);
					const color = lerpColor(left.color, right.color, applyInterpolation('smooth', t));
					parts.push(`${cssColor(color)} ${percent(position)}`);
				}
			}
			parts.push(`${cssColor(right.color)} ${percent(right.position)}`);
		}
		return `linear-gradient(to right, ${parts.join(', ')})`;
	});

	$effect(() => {
		if (stops.length === 0) {
			selected_stop_id = null;
			return;
		}
		if (selected_stop_id === null || !stops.some((stop) => stop.id === selected_stop_id)) {
			selected_stop_id = stops[0].id;
		}
	});

	$effect(() => {
		if (color_editor_stop_id !== null && !stops.some((stop) => stop.id === color_editor_stop_id)) {
			color_editor_stop_id = null;
		}
	});

	const positionFromClientX = (clientX: number): number => {
		if (!trackEl) {
			return 0;
		}
		const rect = trackEl.getBoundingClientRect();
		if (rect.width <= 0) {
			return 0;
		}
		return clamp01((clientX - rect.left) / rect.width);
	};

	const setStopPosition = (stop: StopData, position: number): void => {
		if (!stop.positionParamId) {
			return;
		}
		const value: ParamValue = { kind: 'float', value: clamp01(position) };
		void sendSetParamIntent(stop.positionParamId, value, 'Coalesce');
	};

	const cycleInterpolation = (stop: StopData): void => {
		if (!stop.interpolationParamId) {
			return;
		}
		const currentIndex = INTERPOLATIONS.indexOf(
			stop.interpolation as (typeof INTERPOLATIONS)[number]
		);
		const next = INTERPOLATIONS[(currentIndex + 1 + INTERPOLATIONS.length) % INTERPOLATIONS.length];
		const value: ParamValue = { kind: 'enum', value: next };
		void sendSetParamIntent(stop.interpolationParamId, value, 'Coalesce');
	};

	const openColorEditor = (stop: StopData): void => {
		selected_stop_id = stop.id;
		picker_color = ColorUtil.fromArray(stop.color);
		color_editor_stop_id = stop.id;
	};

	const applyStopColor = (color: Color): void => {
		const stop = stops.find((entry) => entry.id === color_editor_stop_id);
		if (!stop?.colorParamId) {
			return;
		}
		const array = ColorUtil.toArray(color);
		const value: ParamValue = {
			kind: 'color',
			value: [array[0] ?? 0, array[1] ?? 0, array[2] ?? 0, array[3] ?? 1]
		};
		void sendSetParamIntent(stop.colorParamId, value, 'Coalesce');
	};

	// Controlled ColorPicker: it only emits via onchange (it does not write back the bound value
	// when an onchange is provided), so we feed the new color back in and push it to the param.
	const onPickerChange = (color: Color): void => {
		picker_color = color;
		applyStopColor(color);
	};

	const onColorStartEdit = (): void => {
		color_session = createUiEditSession('Edit gradient stop color');
		void color_session.begin();
	};

	const onColorEndEdit = (): void => {
		if (color_session) {
			void color_session.end();
			color_session = null;
		}
	};

	const closeColorEditor = (): void => {
		onColorEndEdit();
		color_editor_stop_id = null;
	};

	// Esc closes the color popup.
	$effect(() => {
		if (color_editor_stop_id === null) {
			return;
		}
		const onKeyDown = (event: KeyboardEvent): void => {
			if (event.key === 'Escape') {
				event.stopPropagation();
				event.preventDefault();
				closeColorEditor();
			}
		};
		window.addEventListener('keydown', onKeyDown, true);
		return () => window.removeEventListener('keydown', onKeyDown, true);
	});

	const addStopAt = async (position: number): Promise<void> => {
		const color = sampleColorAt(position);
		await sendCreateUserItemByTypeIntent(liveNode.node_id, STOP_ITEM_TYPE, 'Stop', {
			initial_params: [
				{ decl_id: DECL_POSITION, value: { kind: 'float', value: clamp01(position) } },
				{
					decl_id: DECL_COLOR,
					value: { kind: 'color', value: [color[0], color[1], color[2], color[3]] }
				}
			]
		});
	};

	const onDragMove = (event: PointerEvent): void => {
		if (dragStopId === null || dragPointerId !== event.pointerId) {
			return;
		}
		const stop = stops.find((entry) => entry.id === dragStopId);
		if (!stop) {
			return;
		}
		if (!dragMoved) {
			const distance = Math.hypot(event.clientX - dragStartX, event.clientY - dragStartY);
			if (distance < DRAG_THRESHOLD_PX) {
				return;
			}
			dragMoved = true;
			dragSession = createUiEditSession('Move gradient stop');
			void dragSession.begin();
		}
		// Dragged far enough below the ribbon arms removal (committed on release while still armed).
		const rect = trackEl?.getBoundingClientRect();
		if (rect && event.clientY > rect.bottom + REMOVE_DRAG_PX) {
			remove_armed_stop_id = stop.id;
			return;
		}
		remove_armed_stop_id = null;
		setStopPosition(stop, positionFromClientX(event.clientX));
	};

	const endDrag = (): void => {
		window.removeEventListener('pointermove', onDragMove);
		window.removeEventListener('pointerup', onDragEnd);
		window.removeEventListener('pointercancel', onDragEnd);
	};

	const onDragEnd = (event: PointerEvent): void => {
		if (dragPointerId !== null && event.pointerId !== dragPointerId) {
			return;
		}
		const stopId = dragStopId;
		const removeOnRelease = stopId !== null && remove_armed_stop_id === stopId;
		if (dragMoved && dragSession) {
			void dragSession.end();
		}
		dragSession = null;
		dragStopId = null;
		dragPointerId = null;
		dragMoved = false;
		remove_armed_stop_id = null;
		endDrag();
		if (removeOnRelease && stopId !== null) {
			void sendRemoveNodeIntent(stopId);
		}
	};

	const onHandlePointerDown = (event: PointerEvent, stop: StopData): void => {
		event.stopPropagation();
		event.preventDefault();
		(event.currentTarget as HTMLElement).focus();
		selected_stop_id = stop.id;
		if (event.altKey) {
			cycleInterpolation(stop);
			return;
		}
		dragStopId = stop.id;
		dragPointerId = event.pointerId;
		dragStartX = event.clientX;
		dragStartY = event.clientY;
		dragMoved = false;
		// Window-level listeners keep the drag alive even when the handle's DOM node is reordered
		// or recreated by stop re-sorting, and when the pointer moves faster than the element.
		window.addEventListener('pointermove', onDragMove);
		window.addEventListener('pointerup', onDragEnd);
		window.addEventListener('pointercancel', onDragEnd);
	};

	// Drop any lingering drag listeners if the editor unmounts mid-drag.
	$effect(() => {
		return () => endDrag();
	});

	const onTrackDoubleClick = (event: MouseEvent): void => {
		if (event.target !== trackEl) {
			return;
		}
		void addStopAt(positionFromClientX(event.clientX));
	};

	const onEditorFocusOut = (event: FocusEvent): void => {
		if (!editorEl?.contains(event.relatedTarget as Node | null)) {
			is_editor_focused = false;
		}
	};

	const removeSelectedStop = async (): Promise<boolean> => {
		if (selected_stop_id === null) {
			return false;
		}
		return sendRemoveNodeIntent(selected_stop_id);
	};

	// Delete the selected stop only while this editor holds focus, mirroring curve-key deletion —
	// local selection never changes the app node selection, so the inspector stays put.
	$effect(() => {
		const unregister = registerCommandHandler(
			'edit.deleteSelection',
			() => {
				if (!is_editor_focused || selected_stop_id === null) {
					return false;
				}
				return removeSelectedStop().then(() => true);
			},
			{ priority: 200 }
		);
		return () => {
			unregister();
		};
	});
</script>

{#snippet gradientHeaderExtra()}
	<span class="gradient-stop-pill">{stop_count_label}</span>
	{#if level > 0}
		<SelectNodeButton {node} />
	{/if}
{/snippet}

{#snippet gradientContent()}
	<div
		class="gradient-node-inspector"
		bind:this={editorEl}
		onfocusin={() => (is_editor_focused = true)}
		onfocusout={onEditorFocusOut}>
		<div class="gradient-preview">
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="gradient-track"
				bind:this={trackEl}
				style:background-image={gradientCss}
				ondblclick={onTrackDoubleClick}
				title="Double-click to add a stop">
				{#each stops as stop (stop.id)}
					<button
						type="button"
						class="gradient-stop-handle"
						class:selected={stop.id === selected_stop_id}
						class:removing={stop.id === remove_armed_stop_id}
						style:left={`${(clamp01(stop.position) * 100).toFixed(2)}%`}
						style:--stop-color={cssColor(stop.color)}
						title={`${Math.round(clamp01(stop.position) * 100)}% · ${stop.interpolation} (drag to move, double-click for color, alt-click for interpolation)`}
						aria-label={`Gradient stop at ${Math.round(clamp01(stop.position) * 100)} percent`}
						onpointerdown={(event) => onHandlePointerDown(event, stop)}
						ondblclick={(event) => {
							event.stopPropagation();
							if (event.altKey) {
								return;
							}
							openColorEditor(stop);
						}}></button>
				{/each}
			</div>
		</div>

		{#if color_editor_stop_id !== null}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="gradient-color-popup-backdrop" onpointerdown={closeColorEditor}></div>
			<div class="gradient-color-popup" role="dialog" aria-label="Stop color">
				<div class="gradient-color-popup-header">
					<span>Stop color</span>
					<button type="button" class="gradient-color-popup-close" onclick={closeColorEditor}
						>×</button>
				</div>
				<ColorPicker
					color={picker_color}
					forceExpanded
					onchange={onPickerChange}
					onStartEdit={onColorStartEdit}
					onEndEdit={onColorEndEdit} />
			</div>
		{/if}

		<div class="gradient-stop-editor">
			{#if selected_stop_node}
				<NodeInspector nodes={[selected_stop_node]} level={level + 1} order="solo" />
			{:else}
				<div class="empty-state">Double-click the ribbon to add a stop.</div>
			{/if}
		</div>
	</div>
{/snippet}

{#if liveNode.node_type === GRADIENT_NODE_TYPE}
	{@render defaultHeader?.(gradientHeaderExtra)}
	{@render defaultContent?.(gradientContent)}
{/if}

<style>
	.gradient-node-inspector {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: 100%;
		max-width: 100%;
		min-inline-size: 0;
		box-sizing: border-box;
	}

	.gradient-preview {
		/* breathing room so the ribbon does not touch the panel edges and edge stops stay in bounds */
		padding-inline: 0.55rem;
		padding-block: 0.15rem 0.55rem;
		box-sizing: border-box;
	}

	.gradient-track {
		position: relative;
		inline-size: 100%;
		block-size: 2rem;
		border-radius: 0.3rem;
		/* Outset ring (not a border) so the gradient reaches the very edges with no white seam on the left. */
		border: none;
		box-shadow: 0 0 0 0.06rem rgba(0, 0, 0, 0.35);
		background-color: #000;
		background-size: cover;
		box-sizing: border-box;
		overflow: visible;
		cursor: copy;
	}

	.gradient-stop-handle {
		position: absolute;
		top: 100%;
		inline-size: 0.5rem;
		block-size: 0.5rem;
		padding: 0;
		transform: translate(-50%, -50%) rotate(45deg);
		border-radius: 0.13rem;
		background: var(--stop-color, #fff);
		border: solid 0.07rem rgba(0, 0, 0, 0.65);
		box-shadow: 0 0 0 0.06rem rgba(255, 255, 255, 0.6);
		cursor: grab;
	}

	.gradient-stop-handle:active {
		cursor: grabbing;
	}

	.gradient-stop-handle.selected {
		box-shadow:
			0 0 0 0.07rem rgba(255, 224, 106, 0.95),
			0 0 0.28rem rgba(255, 224, 106, 0.7);
	}

	.gradient-stop-handle.removing {
		opacity: 0.35;
		border-color: rgba(255, 90, 90, 0.95);
		box-shadow: 0 0 0 0.07rem rgba(255, 90, 90, 0.9);
		cursor: grabbing;
	}

	.gradient-color-popup-backdrop {
		position: fixed;
		inset: 0;
		z-index: 60;
	}

	.gradient-color-popup {
		position: absolute;
		z-index: 61;
		top: 2.7rem;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		padding: 0.45rem;
		border-radius: 0.35rem;
		background: var(--gc-color-surface, #232323);
		border: solid 0.06rem rgba(255, 255, 255, 0.16);
		box-shadow: 0 0.4rem 1.2rem rgba(0, 0, 0, 0.5);
	}

	.gradient-color-popup-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.8rem;
		font-size: 0.62rem;
		opacity: 0.8;
	}

	.gradient-color-popup-close {
		border: none;
		background: transparent;
		color: inherit;
		font-size: 0.95rem;
		line-height: 1;
		cursor: pointer;
		padding: 0 0.2rem;
	}

	.gradient-stop-editor {
		display: flex;
		flex-direction: column;
		min-inline-size: 0;
		overflow: auto;
	}

	.gradient-stop-pill {
		display: inline-flex;
		align-items: center;
		height: 1.02rem;
		padding: 0 0.42rem;
		border-radius: 999rem;
		font-size: 0.6rem;
		background: rgba(119, 196, 255, 0.15);
		border: solid 0.06rem rgba(106, 178, 255, 0.34);
	}

	.empty-state {
		font-size: 0.67rem;
		opacity: 0.68;
		padding: 0.45rem;
		border-radius: 0.28rem;
		background: rgba(255, 255, 255, 0.03);
	}
</style>
