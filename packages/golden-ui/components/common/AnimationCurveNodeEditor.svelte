<script lang="ts">
	import { createCurveOperations } from './curve/operations';
	import { createCurveCreation } from './curve/creation.svelte';
	import { createCurveParameterWriter } from './curve/parameter-writer';
	import { createCurveEditGesture } from './curve/edit-gesture.svelte';
	import { createEditorScopeObserver } from '../../store/editor-scope';
	import { CURVE_NODE_TYPE, CURVE_EPSILON } from './curve/constants';
	import { sample_compiled_curve } from './curve/evaluation';
	import { untrack } from 'svelte';
	import { createCurveInput, curveInputEvents } from './curve/input';
	import { appState } from '../../store/workbench.svelte';
	import { registerCommandHandler } from '../../store/commands.svelte';
	import type { NodeId, UiNodeDto } from '../../types';
	import AnimationCurveCanvas from './AnimationCurveCanvas.svelte';
	import ContextMenu from './ContextMenu.svelte';
	import type { DragPreview, BezierEasingPreview } from './curve/types';
	import type {
		CurveEditorDocument,
		CurveEditorBindings,
		CurveEditorServices
	} from './curve/editor-context';
	import { createCurveEditorState } from './curve/editor-state.svelte';
	import { createCurveEditorSelection } from './curve/editor-selection.svelte';
	import { createCurveEditorPreviews } from './curve/editor-previews';
	import { createCurveEditorConstraints } from './curve/editor-constraints';
	import { createCurveEditorModel } from './curve/editor-model.svelte';
	import { createCurveEditorInsertion } from './curve/editor-insertion';
	import { createCurveEditorSculpt } from './curve/editor-sculpt';
	import { createCurveEditorCamera } from './curve/editor-camera';
	import { createCurveEditorHits } from './curve/editor-hits';
	import { createCurveEditorDrawing } from './curve/editor-drawing';
	import { createCurveEditorCommands } from './curve/editor-commands.svelte';
	import { createCurveEditorPointer } from './curve/editor-pointer';
	let {
		curveNode,
		selected_key_id = $bindable<NodeId | null>(null),
		selected_key_ids = $bindable<NodeId[]>([]),
		selected_curve_owner_key_ids = $bindable<NodeId[]>([]),
		showGrid = true,
		showNumbers = true,
		showBounds = true,
		constrainKeysToNeighbors = true,
		canvasHeight = 'min(26rem, 38vh)',
		onselectionclear
	}: {
		curveNode: UiNodeDto;
		selected_key_id?: NodeId | null;
		selected_key_ids?: NodeId[];
		selected_curve_owner_key_ids?: NodeId[];
		showGrid?: boolean;
		showNumbers?: boolean;
		showBounds?: boolean;
		constrainKeysToNeighbors?: boolean;
		canvasHeight?: string;
		onselectionclear?: () => void;
	} = $props();
	let session = $derived(appState.session);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);
	let input_owner_uuid = $derived(graphNodesById?.get(curveNode.node_id)?.uuid ?? null);
	let input_selection_identity = $derived(
		JSON.stringify([selected_key_ids, selected_curve_owner_key_ids])
	);
	let liveNode: UiNodeDto = $derived(
		session?.graph.state.nodesById.get(curveNode.node_id) ?? curveNode
	);
	const state = createCurveEditorState();
	const doc: CurveEditorDocument = {
		get session() {
			return session;
		},
		get graphNodesById() {
			return graphNodesById;
		},
		get liveNode() {
			return liveNode;
		},
		get input_owner_uuid() {
			return input_owner_uuid;
		},
		get curveNode() {
			return curveNode;
		}
	};
	const bindings: CurveEditorBindings = {
		get selected_key_id() {
			return selected_key_id;
		},
		set selected_key_id(value: typeof selected_key_id) {
			selected_key_id = value;
		},
		get selected_key_ids() {
			return selected_key_ids;
		},
		set selected_key_ids(value: typeof selected_key_ids) {
			selected_key_ids = value;
		},
		get selected_curve_owner_key_ids() {
			return selected_curve_owner_key_ids;
		},
		set selected_curve_owner_key_ids(value: typeof selected_curve_owner_key_ids) {
			selected_curve_owner_key_ids = value;
		},
		get input_selection_identity() {
			return input_selection_identity;
		},
		get constrainKeysToNeighbors() {
			return constrainKeysToNeighbors;
		},
		get onselectionclear() {
			return onselectionclear;
		}
	};
	const services: CurveEditorServices = {
		get curveOperations() {
			return curveOperations;
		},
		get parameterWriter() {
			return parameterWriter;
		},
		get creation() {
			return creation;
		},
		get editGesture() {
			return editGesture;
		},
		get input() {
			return input;
		}
	};
	const context: Parameters<typeof createCurveEditorSelection>[0] &
		Parameters<typeof createCurveEditorPreviews>[0] &
		Parameters<typeof createCurveEditorConstraints>[0] &
		Parameters<typeof createCurveEditorModel>[0] &
		Parameters<typeof createCurveEditorInsertion>[0] &
		Parameters<typeof createCurveEditorSculpt>[0] &
		Parameters<typeof createCurveEditorCamera>[0] &
		Parameters<typeof createCurveEditorHits>[0] &
		Parameters<typeof createCurveEditorDrawing>[0] &
		Parameters<typeof createCurveEditorCommands>[0] &
		Parameters<typeof createCurveEditorPointer>[0] = {
		state,
		doc,
		bindings,
		services,
		get selection(): ReturnType<typeof createCurveEditorSelection> {
			return selection;
		},
		get previews(): ReturnType<typeof createCurveEditorPreviews> {
			return previews;
		},
		get constraints(): ReturnType<typeof createCurveEditorConstraints> {
			return constraints;
		},
		get model(): ReturnType<typeof createCurveEditorModel> {
			return model;
		},
		get insertion(): ReturnType<typeof createCurveEditorInsertion> {
			return insertion;
		},
		get sculpt(): ReturnType<typeof createCurveEditorSculpt> {
			return sculpt;
		},
		get camera(): ReturnType<typeof createCurveEditorCamera> {
			return camera;
		},
		get hits(): ReturnType<typeof createCurveEditorHits> {
			return hits;
		},
		get drawing(): ReturnType<typeof createCurveEditorDrawing> {
			return drawing;
		},
		get commands(): ReturnType<typeof createCurveEditorCommands> {
			return commands;
		}
	};
	const selection = createCurveEditorSelection(context);
	const previews = createCurveEditorPreviews(context);
	const constraints = createCurveEditorConstraints(context);
	const model = createCurveEditorModel(context);
	const insertion = createCurveEditorInsertion(context);
	const sculpt = createCurveEditorSculpt(context);
	const camera = createCurveEditorCamera(context);
	const hits = createCurveEditorHits(context);
	const drawing = createCurveEditorDrawing(context);
	const commands = createCurveEditorCommands(context);
	const pointer = createCurveEditorPointer(context);

	const curveOperations = createCurveOperations({
		get session() {
			return session;
		},
		get owner() {
			return liveNode;
		}
	});
	const parameterWriter = createCurveParameterWriter({
		get nodes() {
			return graphNodesById;
		},
		key: (id) => model.parsed_key_by_id.get(id),
		bezier: (id) => sculpt.bezier_param_refs_by_key_id(id)
	});
	const creation = createCurveCreation({
		ready: insertion.pending_create_targets_ready,
		preview(plan, key) {
			const ids = new Set([
				key.node_id,
				...plan.existing_easing_updates.map((value) => value.key_id),
				...(plan.split_easing
					? [
							plan.split_easing.source_start_key_id,
							...(plan.split_easing.preserved_aligned_handle_updates ?? []).map(
								(value) => value.key_id
							)
						]
					: [])
			]);
			const beforeKeys = new Map([...ids].map((id) => [id, state.drag_preview_by_key_id.get(id)]));
			const beforeEasing = new Map(
				[...ids].map((id) => [id, state.easing_drag_preview_by_key_id.get(id)])
			);
			previews.sync_pending_create_previews(plan, key.node_id);
			const keys = new Map<NodeId, DragPreview>(),
				easing = new Map<NodeId, BezierEasingPreview>();
			for (const id of ids) {
				const keyPreview = state.drag_preview_by_key_id.get(id),
					easingPreview = state.easing_drag_preview_by_key_id.get(id);
				if (keyPreview && keyPreview !== beforeKeys.get(id)) keys.set(id, keyPreview);
				if (easingPreview && easingPreview !== beforeEasing.get(id)) easing.set(id, easingPreview);
			}
			return () => {
				previews.clear_drag_previews_if_unchanged(keys);
				previews.clear_easing_drag_previews_if_unchanged(easing);
			};
		},
		async initialize(scope, plan, key, keys) {
			await parameterWriter.numeric(key.position_param, plan.position, scope);
			await parameterWriter.numeric(key.value_param, plan.value, scope);
			if (plan.split_easing) {
				await commands.apply_easing_param_values(
					keys.get(plan.split_easing.source_start_key_id),
					plan.split_easing.left_param_values,
					scope
				);
				await commands.apply_easing_param_values(key, plan.split_easing.right_param_values, scope);
				for (const update of plan.split_easing.preserved_aligned_handle_updates ?? []) {
					await commands.apply_easing_param_values(keys.get(update.key_id), update.values, scope);
				}
			} else if (plan.easing_values) {
				await commands.apply_easing_param_values(key, plan.easing_values, scope);
			}
			for (const update of plan.existing_easing_updates) {
				await commands.apply_easing_param_values(keys.get(update.key_id), update.values, scope);
			}
		},
		select(scope, key) {
			if (!scope.selectionCurrent()) return;
			selection.set_single_selected_key(key.node_id);
			const node = scope.host.graph.state.nodesById.get(key.node_id);
			if (node) scope.host.requestNodeSelection([{ node_id: node.node_id, uuid: node.uuid }]);
		}
	});
	$effect(() => {
		if (typeof window === 'undefined') {
			return;
		}
		const refresh = (): void => {
			const parsed = Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize);
			if (Number.isFinite(parsed) && parsed > 0) {
				state.rem_base_px = parsed;
			}
		};
		refresh();
		window.addEventListener('resize', refresh);
		return () => {
			window.removeEventListener('resize', refresh);
		};
	});
	$effect(() => {
		if (model.parsed_keys.length === 0) {
			if (selected_key_id !== null) {
				selected_key_id = null;
			}
			if (selected_key_ids.length > 0) {
				selected_key_ids = [];
			}
			return;
		}
		const available_key_ids = new Set(model.parsed_keys.map((entry) => entry.node_id));
		const filtered_selection = selected_key_ids.filter((key_id) => available_key_ids.has(key_id));
		if (filtered_selection.length === 0) {
			if (selected_key_id !== null && available_key_ids.has(selected_key_id)) {
				const singleton = [selected_key_id];
				if (!selection.same_node_id_array(selected_key_ids, singleton)) {
					selected_key_ids = singleton;
				}
				return;
			}
			if (selected_key_id !== null) {
				selected_key_id = null;
			}
			if (selected_key_ids.length > 0) {
				selected_key_ids = [];
			}
			return;
		}
		if (!selection.same_node_id_array(filtered_selection, selected_key_ids)) {
			selected_key_ids = filtered_selection;
		}
		if (
			(selected_key_id === null || !filtered_selection.includes(selected_key_id)) &&
			selected_key_id !== filtered_selection[0]
		) {
			selected_key_id = filtered_selection[0];
		}
	});
	$effect(() => {
		if (model.parsed_keys.length === 0) {
			if (selected_curve_owner_key_ids.length > 0) {
				selected_curve_owner_key_ids = [];
			}
			if (state.hover_curve_owner_key_id !== null) {
				state.hover_curve_owner_key_id = null;
			}
			return;
		}
		const available_key_ids = new Set(model.parsed_keys.map((entry) => entry.node_id));
		const filtered_selection = selected_curve_owner_key_ids.filter((key_id) =>
			available_key_ids.has(key_id)
		);
		if (!selection.same_node_id_array(filtered_selection, selected_curve_owner_key_ids)) {
			selected_curve_owner_key_ids = filtered_selection;
		}
		if (
			state.hover_curve_owner_key_id !== null &&
			!available_key_ids.has(state.hover_curve_owner_key_id)
		) {
			state.hover_curve_owner_key_id = null;
		}
	});
	$effect(() => {
		if (model.compiled_curve.keys.length !== 0) {
			return;
		}
		if (state.fixed_view_bounds !== null) {
			camera.cancel_view_bounds_animation();
			state.fixed_view_bounds = null;
		}
	});
	$effect(() => {
		model.active_curve_range_constraint;
		const current_fixed = state.fixed_view_bounds;
		if (!current_fixed) {
			return;
		}
		const sanitized = camera.sanitize_view_bounds({ ...current_fixed });
		if (!sanitized) {
			camera.cancel_view_bounds_animation();
			state.fixed_view_bounds = null;
			return;
		}
		if (
			Math.abs(current_fixed.x_min - sanitized.x_min) > CURVE_EPSILON ||
			Math.abs(current_fixed.x_max - sanitized.x_max) > CURVE_EPSILON ||
			Math.abs(current_fixed.y_min - sanitized.y_min) > CURVE_EPSILON ||
			Math.abs(current_fixed.y_max - sanitized.y_max) > CURVE_EPSILON
		) {
			state.fixed_view_bounds = sanitized;
		}
	});
	$effect(() => {
		model.compiled_curve;
		model.active_curve_range_constraint;
		if (state.fixed_view_bounds !== null) {
			return;
		}
		const next_bounds = camera.default_curve_view_bounds();
		if (next_bounds) {
			state.fixed_view_bounds = next_bounds;
		}
	});
	const editGesture = createCurveEditGesture({
		get session() {
			return session;
		},
		get nodes() {
			return graphNodesById;
		},
		get owner() {
			return liveNode;
		},
		get selection() {
			return input_selection_identity;
		},
		get busy() {
			return creation.busy;
		},
		writer: parameterWriter,
		cancel() {
			input.cancel();
		}
	});
	const input = createCurveInput({
		get canvas() {
			return state.canvas_element;
		},
		get key() {
			return state.active_drag;
		},
		set key(value) {
			state.active_drag = value;
		},
		get bezier() {
			return state.active_bezier_handle_drag;
		},
		set bezier(value) {
			state.active_bezier_handle_drag = value;
		},
		get curve() {
			return state.active_curve_drag;
		},
		set curve(value) {
			state.active_curve_drag = value;
		},
		get pan() {
			return state.active_canvas_pan;
		},
		set pan(value) {
			state.active_canvas_pan = value;
		},
		get draw() {
			return state.active_curve_draw;
		},
		set draw(value) {
			state.active_curve_draw = value;
		},
		get box() {
			return state.active_box_selection;
		},
		set box(value) {
			state.active_box_selection = value;
		},
		get keyPreviews() {
			return state.drag_preview_by_key_id;
		},
		get easingPreviews() {
			return state.easing_drag_preview_by_key_id;
		},
		flush() {
			editGesture.flush();
			editGesture.discard();
		},
		discard: editGesture.discard,
		takeEdit: editGesture.take,
		clearPreviews(keys, easing) {
			previews.clear_drag_previews_if_unchanged(keys);
			previews.clear_easing_drag_previews_if_unchanged(easing);
		},
		completeBox: selection.complete_box_selection,
		completeDraw(points) {
			void drawing.commit_curve_draw(points);
		},
		restoreView: camera.set_view_bounds_instantly,
		clearHover() {
			state.hover_key_id = null;
			state.hover_curve_owner_key_id = null;
			state.hover_bezier_handle = null;
			state.hover_curve_position = null;
		}
	});
	$effect(() => {
		if (!state.curve_context_menu_open) {
			return;
		}
		if (state.curve_context_menu_anchor && commands.curve_context_menu_items.length > 0) {
			return;
		}
		commands.close_curve_context_menu();
	});
	$effect(() => {
		const unregisterHandlers = [
			registerCommandHandler(
				'edit.deleteSelection',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					return commands.remove_selected_keys().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'select.all',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					selection.set_all_keys_selected();
					return true;
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.copy',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					commands.copy_selected_keys_to_clipboard();
					return true;
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.cut',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					return commands.cut_selected_keys_to_clipboard().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.duplicate',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					return commands.duplicate_selected_keys().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.paste',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					return commands.paste_clipboard_keys().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'view.frame',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					return camera.frame_selected_view();
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'view.home',
				() => {
					if (!state.is_canvas_focused) {
						return false;
					}
					camera.home_view();
					return true;
				},
				{ priority: 200 }
			)
		];
		return () => {
			for (const unregister of unregisterHandlers) {
				unregister();
			}
		};
	});
	$effect(() => {
		const canvas = state.canvas_element;
		if (!canvas) return;
		return curveInputEvents(canvas, {
			cancel(pointer) {
				const menu = pointer === undefined && state.curve_context_menu_open;
				if (menu) commands.close_curve_context_menu();
				const retired = input.cancel(pointer);
				if (retired) state.last_canvas_primary_down = null;
				return menu || retired;
			},
			clear: selection.clear_canvas_selection
		});
	});
	const observeInputScope = createEditorScopeObserver(() => {
		input.cancel();
		editGesture.discard();
		camera.cancel_view_bounds_animation();
		pointer.on_canvas_pointer_leave();
	});
	$effect(() => {
		const scope = {
			host: session,
			project: session?.projectGeneration ?? null,
			snapshot: session?.snapshotGeneration ?? 0,
			status: session?.status ?? 'disconnected',
			loading: session?.loading.visible ?? true,
			owner: input_owner_uuid
		};
		const scope_owner = scope.owner;
		untrack(() => {
			curveOperations.observe();
			observeInputScope(scope);
		});
		if (state.input_view_owner !== scope_owner)
			untrack(() => {
				state.input_view_owner = scope_owner;
				state.fixed_view_bounds = null;
			});
	});
	$effect(() => {
		return () => {
			curveOperations.dispose();
			input.cancel();
			editGesture.discard();
			camera.cancel_view_bounds_animation();
		};
	});
</script>

{#if liveNode.node_type === CURVE_NODE_TYPE}
	<div class="animation-curve-node-editor">
		<div class="curve-canvas-shell" style:height={canvasHeight}>
			<AnimationCurveCanvas
				compiledCurve={model.compiled_curve}
				parsedKeys={model.parsed_keys}
				selectedKeyId={selected_key_id}
				selectedKeyIds={selected_key_ids}
				selectedCurveOwnerKeyIds={selected_curve_owner_key_ids}
				hoverKeyId={state.hover_key_id}
				hoverCurveOwnerKeyId={state.hover_curve_owner_key_id}
				bezierHandles={model.bezier_handle_visuals}
				activeBezierHandle={state.active_bezier_handle_drag
					? state.active_bezier_handle_drag.ref
					: null}
				hoverBezierHandle={state.hover_bezier_handle}
				hoverCurvePosition={state.hover_curve_position}
				drawPathPoints={model.active_curve_draw_points}
				selectionRect={selection.active_box_selection_rect}
				activeCurveRangeConstraint={model.active_curve_range_constraint}
				fixedViewBounds={state.fixed_view_bounds}
				bind:interactionTransform={state.interaction_transform}
				bind:canvasElement={state.canvas_element}
				activeCanvasPan={state.active_canvas_pan !== null}
				{showGrid}
				{showNumbers}
				{showBounds}
				canvasHeight="100%"
				sampleCurveAt={(position) => sample_compiled_curve(model.compiled_curve, position)}
				onpointerdown={pointer.on_canvas_pointer_down}
				onpointermove={pointer.on_canvas_pointer_move}
				onpointerup={pointer.on_canvas_pointer_up}
				onpointerleave={pointer.on_canvas_pointer_leave}
				onwheel={pointer.on_canvas_wheel}
				oncontextmenu={pointer.on_canvas_context_menu}
				onfocus={pointer.on_canvas_focus}
				onblur={pointer.on_canvas_blur} />
		</div>
		<ContextMenu
			bind:open={state.curve_context_menu_open}
			items={commands.curve_context_menu_items}
			anchor={state.curve_context_menu_anchor}
			insideElements={[state.canvas_element]}
			minWidthRem={10}
			maxWidthCss="min(18rem, calc(100vw - 2rem))"
			zIndex={60}
			onClose={commands.close_curve_context_menu} />
	</div>
{/if}

<style>
	.animation-curve-node-editor {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		min-height: 0;
	}

	.curve-canvas-shell {
		height: min(26rem, 38vh);
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}
</style>
