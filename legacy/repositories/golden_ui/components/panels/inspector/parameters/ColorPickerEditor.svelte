<script lang="ts">
	import { onDestroy } from 'svelte';
	import ColorPicker from '../../../common/ColorPicker.svelte';
	import { appState } from '../../../../store/workbench.svelte';
	import { createUiEditSession, sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto } from '../../../../types';

	interface ColorEditorPresentation {
		force_expanded?: boolean;
		show_hex?: boolean;
		show_rgba_fields?: boolean;
	}

	let {
		node,
		layoutMode = 'default',
		presentation = {},
		readOnly: externalReadOnly = false
	} = $props<{
		node: UiNodeDto;
		layoutMode?: 'default' | 'widget';
		presentation?: ColorEditorPresentation;
		readOnly?: boolean;
	}>();

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
	let enabled = $derived(liveNode.meta.enabled);
	let value = $derived.by((): [number, number, number, number] => {
		if (param?.value.kind !== 'color') {
			return [0, 0, 0, 1];
		}
		const [red, green, blue, alpha] = param.value.value;
		return [red, green, blue, alpha];
	});

	let editSession = createUiEditSession('Edit color', 'param-color');
	let isEditing = $state(false);
	let draftColor = $state<[number, number, number, number]>([0, 0, 0, 1]);

	$effect(() => {
		if (isEditing) {
			return;
		}
		draftColor = value;
	});

	onDestroy(() => {
		void editSession.end();
	});

	const clamp01 = (valueToClamp: number): number => {
		return Math.min(1, Math.max(0, valueToClamp));
	};

	const toColorArray = (nextColor: unknown): [number, number, number, number] => {
		if (Array.isArray(nextColor)) {
			return [
				clamp01(Number(nextColor[0] ?? 0)),
				clamp01(Number(nextColor[1] ?? 0)),
				clamp01(Number(nextColor[2] ?? 0)),
				clamp01(Number(nextColor[3] ?? 1))
			];
		}
		if (nextColor && typeof nextColor === 'object') {
			const candidate = nextColor as { r?: unknown; g?: unknown; b?: unknown; a?: unknown };
			return [
				clamp01(Number(candidate.r ?? 0)),
				clamp01(Number(candidate.g ?? 0)),
				clamp01(Number(candidate.b ?? 0)),
				clamp01(Number(candidate.a ?? 1))
			];
		}
		return [0, 0, 0, 1];
	};

	const startEdit = (): void => {
		if (readOnly || !enabled) {
			return;
		}
		isEditing = true;
		void editSession.begin();
	};

	const updateValue = (nextColor: unknown): void => {
		if (!param || param.value.kind !== 'color' || readOnly || !enabled) {
			return;
		}
		const normalized = toColorArray(nextColor);
		draftColor = normalized;
		void sendSetParamIntent(
			liveNode.node_id,
			{ kind: 'color', value: normalized },
			param.event_behaviour
		);
	};

	const endEdit = (): void => {
		if (!isEditing) {
			return;
		}
		isEditing = false;
		void editSession.end();
	};
</script>

<div class="color-picker-editor" class:widget-layout={layoutMode === 'widget'}>
	<ColorPicker
		previewIsSwitch={enabled && !readOnly}
		forceExpanded={presentation.force_expanded === true}
		showHex={presentation.show_hex !== false}
		showRgbaFields={presentation.show_rgba_fields !== false}
		{layoutMode}
		color={draftColor}
		onchange={(nextColor: unknown) => {
			updateValue(nextColor);
		}}
		onStartEdit={startEdit}
		onEndEdit={endEdit}></ColorPicker>
</div>

<style>
	.color-picker-editor {
		display: flex;
		justify-content: right;
		inline-size: 100%;
		min-inline-size: 0;
	}

	.color-picker-editor.widget-layout {
		block-size: 100%;
		min-block-size: 0;
	}
</style>
