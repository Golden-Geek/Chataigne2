import { onDestroy } from 'svelte';
import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
import { usePanelVisibility } from 'golden_ui/dockview/panel-visibility.svelte';
import { createEditorParameterProjection } from 'golden_ui/store/editor-parameter-projection.svelte';
import { projectInstanceParameters } from './parameter-projection';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { createLatestEditGestureState } from 'golden_ui/store/latest-edit-gesture';
import { createCompositionEditorState, CompositionEditorController } from './editor-controller';
import {
	createInstanceGesture,
	type InstanceHandle,
	type InstancePresentation
} from './instance-gesture';
import { invokeMediaEdit } from '../media/operations';
import { MEDIA_NAMESPACE } from '../media/generated/constants';
import type { CompositionEditorQuery } from '../media/generated/CompositionEditorQuery';
import type { InstanceReference } from '../media/generated/InstanceReference';
import type { InstanceId } from '../media/generated/InstanceId';

export function createCompositionEditor(query: () => CompositionEditorQuery) {
	const panel = usePanelVisibility();
	const session = $derived(appState.session);
	const view = $state(createCompositionEditorState());
	const controller = new CompositionEditorController<WorkbenchSession>(
		() => session,
		view,
		invokeMediaEdit
	);
	const gestureState = $state(createLatestEditGestureState());
	const projection = createEditorParameterProjection(() => session);
	const parameters = $derived(
		panel.visible && view.page ? projection.read(view.presentedGeneration, view.page.at) : null
	);
	const bindings = $derived(
		new Map(view.page?.parameters.map((binding) => [binding.instance, binding]) ?? [])
	);
	const project = (instance: NonNullable<typeof controller.instance>) =>
		parameters && !view.interacting
			? projectInstanceParameters(instance, bindings.get(instance.reference.instance), parameters)
			: instance;
	const composition = $derived.by(() => {
		const value = controller.composition;
		return value ? { ...value, instances: value.instances.map(project) } : null;
	});
	const instance = $derived(controller.instance ? project(controller.instance) : null);
	let activeInstance = $state<InstanceId | null>(null);
	let presentation = $state.raw<InstancePresentation | null>(null);
	const gesture = createInstanceGesture<WorkbenchSession>(() => session, gestureState, {
		group: (session, handle) =>
			createUiEditSession(
				`${typeof handle.mode === 'string' ? handle.mode : 'Resize'} instance`,
				'composition-geometry',
				session
			),
		invoke: (current, edit) => invokeMediaEdit(current, edit, projection.applied()),
		accepted: (snapshot) => controller.acceptInstance(snapshot),
		present: (value) => {
			presentation = value;
		},
		active: (active) => {
			controller.setInteracting(active);
			if (!active) activeInstance = null;
		},
		finished: () => controller.refresh()
	});
	observeEditor({
		query,
		topics: [MEDIA_NAMESPACE],
		pending: () => view.pending,
		interacting: () => view.interacting,
		owners: () => [
			...(controller.channel ? [controller.channel.reference.media] : []),
			...(controller.composition ? [controller.composition.reference.media] : [])
		],
		refresh: (request, resolve) => controller.refresh(request, resolve),
		observe: (shown, request) => {
			gesture.observe();
			if (!shown || (activeInstance && request.selected !== activeInstance)) gesture.stop();
		}
	});
	onDestroy(() => {
		gesture.dispose();
		controller.dispose();
	});
	return {
		view,
		controller,
		gestureState,
		gesture,
		get presentation() {
			return presentation;
		},
		get channel() {
			return controller.channel;
		},
		get composition() {
			return composition;
		},
		get instance() {
			return instance;
		},
		get busy() {
			return controller.busy || session?.hasActiveEditSession === true;
		},
		start(reference: InstanceReference, handle: InstanceHandle) {
			if (!gesture.start(reference, handle)) return false;
			activeInstance = reference.instance;
			return true;
		}
	};
}
